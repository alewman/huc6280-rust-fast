// SPDX-License-Identifier: MIT
//! The public HuC6280 CPU (`cpu.py`): the frame's four hooks, the step, and
//! state capture.
//!
//! The host owns memory and every device: it gives the CPU a [`Bus`], calls
//! [`HuC6280::step`], and runs its devices inside the callbacks for as long
//! as each cycle lasts on its board. See the reference's
//! docs/handoff-brief.md, "The bus contract".
//!
//! Differences forced by Rust: the reference's class is nine mixins on
//! `CPUFrame`; here each mixin is an `impl` block in its own module and the
//! frame is a field ([`Pins::frame`]) reached through 6502-rust's [`Part`]
//! trait, whose provided `step()` is the frame's.

use sixfiveohtwo::frame::{Frame, Part};

use crate::control::{IRQ1_BIT, IRQ2_BIT, TIQ_BIT};
use crate::core::{Bus, HuC6280, T};
use crate::state::CpuState;

impl<B: Bus> Part for HuC6280<B> {
    fn frame(&self) -> &Frame {
        &self.pins.frame
    }

    fn frame_mut(&mut self) -> &mut Frame {
        &mut self.pins.frame
    }

    fn status(&self) -> u8 {
        self.p
    }

    fn accept_reset(&mut self) -> u32 {
        self.reset_sequence()
    }

    fn halted_cycle(&mut self) -> u32 {
        unimplemented!("the HuC6280 has no halted state")
    }

    fn enter_interrupt(&mut self, kind: u8) -> u32 {
        self.enter_interrupt_sequence(kind)
    }

    fn execute(&mut self) -> u32 {
        self.execute_sequence()
    }

    fn boundary_interrupt(&mut self) -> u8 {
        self.boundary_interrupt_sequence()
    }
}

impl<B: Bus> HuC6280<B> {
    /// One instruction: the fetch, then the handler's cycles (`_execute`).
    pub(crate) fn execute_sequence(&mut self) -> u32 {
        self.cycles = 0;
        let pc = self.pc;
        let opcode = self.cycle_read(pc);
        self.pc = pc.wrapping_add(1);
        self.pins.opcode = opcode;
        // U4: the memory-operation flag is cleared in the fetch cycle of the
        // next instruction; the handler sees what it was.
        self.t = self.p & T;
        self.p &= !T;
        self.dispatch_opcode(opcode);
        // A line that changed after the last bus access changed in the last
        // cycle, after the poll (U21): the next boundary decides on the lines
        // as they stood at that access. The frame keeps only whether an
        // enabled line was asserted; a swap of sources leaves that unchanged
        // (issue 0005).
        let pins = &mut self.pins;
        pins.polled_lines = if pins.frame.deferred.is_some() && pins.lines != pins.lines_at_access {
            Some(pins.lines_at_access)
        } else {
            None
        };
        self.cycles
    }

    /// Execute one instruction, interrupt entry or reset; return its cycle
    /// count ([`Part::step`], the frame's).
    ///
    /// The fork: the usual boundary (no reset, not halted, nothing requested,
    /// nothing the last poll left: `Part::step` would skip the interrupt
    /// decision too) is tested in one go and runs the instruction here; any
    /// other boundary is `Part::step`'s. The polled lines (issue 0005) need
    /// no test of their own: `Part::step` skips the boundary decision that
    /// would use them up at such a boundary too, and `execute_sequence`
    /// rewrites them at its end on either path.
    #[inline]
    pub fn step(&mut self) -> u32 {
        if B::QUIET {
            // the fork's quiet window (not kept): ask for one, pay what was spent at the end
            self.window = self.bus.quiet_window(&self.pins);
            let cycles = Part::step(self);
            self.bus
                .quiet_spent(std::mem::take(&mut self.spent), &self.pins);
            self.window.budget = i64::MIN;
            return cycles;
        }
        let frame = &self.pins.frame;
        if frame.reset_pending
            | frame.halted
            | (frame.requests != 0)
            | frame.polled.is_some()
            | frame.polled_i.is_some()
        {
            return self.step_at_a_request();
        }
        let cycles = self.execute_sequence();
        let frame = &mut self.pins.frame;
        if let Some(deferred) = frame.deferred {
            // A bus callback changed a request in the last cycle, after the poll.
            frame.deferred = None;
            frame.polled = Some(deferred);
        }
        cycles
    }

    /// A boundary with something to decide: the frame's own step.
    #[inline(never)]
    fn step_at_a_request(&mut self) -> u32 {
        Part::step(self)
    }

    /// Run the start sequence at the next boundary (the frame's).
    pub fn request_reset(&mut self) {
        self.pins.frame.request_reset();
    }

    /// Latch an NMI edge (the frame's).
    pub fn request_non_maskable_interrupt(&mut self) {
        self.pins.frame.request_non_maskable_interrupt();
    }

    // -- state -----------------------------------------------------------

    /// Return a snapshot of all CPU-owned state at a boundary.
    ///
    /// No host reads, no side effects. Memory and devices are the host's and
    /// are not included.
    pub fn capture_state(&self) -> CpuState {
        let (irq1, irq2, tiq) = self.pins.irq_lines();
        let poll = self.pins.frame.poll_fields();
        CpuState {
            a: self.a,
            x: self.x,
            y: self.y,
            s: self.s,
            pc: self.pc,
            p: self.p,
            mpr: self.mpr,
            high_speed: self.pins.high_speed,
            mpr_latch: self.mpr_latch,
            irq1,
            irq2,
            tiq,
            irq_disable: self.pins.irq_disable,
            halted: poll.halted,
            reset_pending: poll.reset_pending,
            maskable_interrupt_pending: poll.maskable_interrupt_pending,
            non_maskable_interrupt_pending: poll.non_maskable_interrupt_pending,
            polled_i: poll.polled_i,
            polled_maskable_interrupt: poll.polled_maskable_interrupt,
            polled_non_maskable_interrupt: poll.polled_non_maskable_interrupt,
            skipped_poll: poll.skipped_poll,
            polled_irq_lines: self.pins.polled_lines,
        }
    }

    /// Restore a captured CPU state without touching the host.
    pub fn restore_state(&mut self, state: &CpuState) -> Result<(), String> {
        state.validate()?;
        self.a = state.a;
        self.x = state.x;
        self.y = state.y;
        self.s = state.s;
        self.pc = state.pc;
        self.p = state.p;
        self.mpr = state.mpr;
        let pins = &mut self.pins;
        pins.high_speed = state.high_speed;
        self.mpr_latch = state.mpr_latch;
        pins.lines =
            (if state.irq1 { IRQ1_BIT } else { 0 }) | (if state.irq2 { IRQ2_BIT } else { 0 });
        pins.lines |= if state.tiq { TIQ_BIT } else { 0 };
        pins.poll_lines = pins.lines;
        pins.polled_lines = state.polled_irq_lines;
        pins.irq_disable = state.irq_disable;
        pins.frame.restore_poll(&state.poll_fields());
        // The reference's _restore_poll ends in its _confirm, which sets
        // _lines_at_access; Frame::restore_poll cannot reach the lines.
        pins.lines_at_access = pins.lines;
        Ok(())
    }
}
