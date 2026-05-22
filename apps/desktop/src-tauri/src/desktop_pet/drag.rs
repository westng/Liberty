#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DragPoint {
    pub(crate) x: i32,
    pub(crate) y: i32,
}

impl DragPoint {
    pub(crate) fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PetDragMachine {
    mouse_down_at: Option<DragPoint>,
    window_down_at: Option<DragPoint>,
    drag_started: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PetDragMove {
    None,
    MoveTo(DragPoint),
}

impl PetDragMachine {
    const START_THRESHOLD_PX: i32 = 4;

    pub(crate) fn record_mouse_down(&mut self, cursor: DragPoint, window: DragPoint) {
        self.mouse_down_at = Some(cursor);
        self.window_down_at = Some(window);
        self.drag_started = false;
    }

    pub(crate) fn pointer_moved(&mut self, current: DragPoint) -> PetDragMove {
        let Some(start) = self.mouse_down_at else {
            return PetDragMove::None;
        };
        let Some(window_start) = self.window_down_at else {
            return PetDragMove::None;
        };

        let delta_x = current.x - start.x;
        let delta_y = current.y - start.y;
        if !self.drag_started
            && delta_x.abs() < Self::START_THRESHOLD_PX
            && delta_y.abs() < Self::START_THRESHOLD_PX
        {
            return PetDragMove::None;
        }

        self.drag_started = true;
        PetDragMove::MoveTo(DragPoint {
            x: window_start.x + delta_x,
            y: window_start.y + delta_y,
        })
    }

    pub(crate) fn finish_mouse_interaction(&mut self) -> bool {
        let should_interact = self.mouse_down_at.is_some() && !self.drag_started;
        self.reset();
        should_interact
    }

    pub(crate) fn reset(&mut self) {
        self.mouse_down_at = None;
        self.window_down_at = None;
        self.drag_started = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_motion_below_threshold_stays_click_interaction() {
        let mut machine = PetDragMachine::default();
        machine.record_mouse_down(DragPoint::new(100, 100), DragPoint::new(40, 50));

        assert_eq!(
            machine.pointer_moved(DragPoint::new(103, 103)),
            PetDragMove::None
        );
        assert!(machine.finish_mouse_interaction());
    }

    #[test]
    fn pointer_motion_at_threshold_starts_window_drag() {
        let mut machine = PetDragMachine::default();
        machine.record_mouse_down(DragPoint::new(100, 100), DragPoint::new(40, 50));

        assert_eq!(
            machine.pointer_moved(DragPoint::new(104, 100)),
            PetDragMove::MoveTo(DragPoint::new(44, 50))
        );
        assert!(!machine.finish_mouse_interaction());
    }

    #[test]
    fn started_drag_keeps_moving_for_each_pointer_update() {
        let mut machine = PetDragMachine::default();
        machine.record_mouse_down(DragPoint::new(100, 100), DragPoint::new(40, 50));

        assert_eq!(
            machine.pointer_moved(DragPoint::new(110, 112)),
            PetDragMove::MoveTo(DragPoint::new(50, 62))
        );
        assert_eq!(
            machine.pointer_moved(DragPoint::new(130, 125)),
            PetDragMove::MoveTo(DragPoint::new(70, 75))
        );
        assert!(!machine.finish_mouse_interaction());
    }

    #[test]
    fn reset_clears_drag_capture_state() {
        let mut machine = PetDragMachine::default();
        machine.record_mouse_down(DragPoint::new(100, 100), DragPoint::new(40, 50));
        assert_eq!(
            machine.pointer_moved(DragPoint::new(110, 110)),
            PetDragMove::MoveTo(DragPoint::new(50, 60))
        );

        machine.reset();

        assert_eq!(
            machine.pointer_moved(DragPoint::new(120, 120)),
            PetDragMove::None
        );
        assert!(!machine.finish_mouse_interaction());
    }
}
