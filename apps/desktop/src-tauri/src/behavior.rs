#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BehaviorEvent {
    StartWalk,
    StopWalk,
    FallAsleep,
    Wake,
    Tap,
    DragStart,
    DragEnd,
    AnimationFinished,
}

pub fn transition(current: &str, event: BehaviorEvent) -> &'static str {
    match event {
        BehaviorEvent::Tap => "tap",
        BehaviorEvent::DragStart => "drag",
        BehaviorEvent::DragEnd if current == "drag" => "drop",
        BehaviorEvent::AnimationFinished if matches!(current, "tap" | "drop") => "idle",
        BehaviorEvent::StartWalk if current == "idle" => "walk",
        BehaviorEvent::StopWalk if current == "walk" => "idle",
        BehaviorEvent::FallAsleep if current == "idle" => "sleep",
        BehaviorEvent::Wake if current == "sleep" => "idle",
        _ => normalize(current),
    }
}

pub fn normalize(current: &str) -> &'static str {
    match current {
        "idle" => "idle",
        "walk" => "walk",
        "sleep" => "sleep",
        "tap" => "tap",
        "drag" => "drag",
        "drop" => "drop",
        _ => "idle",
    }
}

#[cfg(test)]
mod tests {
    use super::{BehaviorEvent, normalize, transition};

    #[test]
    fn supports_idle_walk_and_sleep_transitions() {
        assert_eq!(transition("idle", BehaviorEvent::StartWalk), "walk");
        assert_eq!(transition("walk", BehaviorEvent::StopWalk), "idle");
        assert_eq!(transition("idle", BehaviorEvent::FallAsleep), "sleep");
        assert_eq!(transition("sleep", BehaviorEvent::Wake), "idle");
    }

    #[test]
    fn tap_interrupts_any_state_then_returns_idle() {
        for state in ["idle", "walk", "sleep", "tap", "drop"] {
            assert_eq!(transition(state, BehaviorEvent::Tap), "tap");
            assert_eq!(transition("tap", BehaviorEvent::AnimationFinished), "idle");
        }
    }

    #[test]
    fn drag_and_drop_have_explicit_recovery() {
        for state in ["idle", "walk", "sleep", "tap", "drop"] {
            assert_eq!(transition(state, BehaviorEvent::DragStart), "drag");
        }
        assert_eq!(transition("drag", BehaviorEvent::DragEnd), "drop");
        assert_eq!(transition("drop", BehaviorEvent::AnimationFinished), "idle");
    }

    #[test]
    fn invalid_persisted_states_are_normalized() {
        assert_eq!(normalize("paused"), "idle");
        assert_eq!(normalize("unknown"), "idle");
    }
}
