use serde::{Deserialize, Serialize};

/// Symmetrical peer control state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    /// Local user is using local peripherals on the local desktop.
    LocalActive,
    /// Local user has moved pointer beyond boundary and is controlling a remote machine.
    /// (Local physical inputs are captured and suppressed, then transmitted across network).
    ControllingRemote {
        target_peer_id: String,
    },
    /// This machine is currently receiving synthetic inputs from a remote master.
    /// (Any physical mouse/keyboard movement locally will immediately preempt this).
    RemoteControlled {
        controller_peer_id: String,
    },
}

impl Default for SessionState {
    fn default() -> Self {
        SessionState::LocalActive
    }
}

/// Action to execute on state transition
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateAction {
    None,
    /// Hide local cursor and start suppressing local input
    TrapAndSuppressLocal,
    /// Unhide local cursor and resume normal local event delivery
    ReleaseAndUnhideLocal,
    /// Send PreemptTakeover packet to controller peer
    NotifyPreempt { peer_id: String },
    /// Send EnterScreen to target peer
    NotifyEnter { peer_id: String },
    /// Send LeaveScreen to target peer
    NotifyLeave { peer_id: String },
}

/// Thread-safe state coordinator
#[derive(Debug, Default)]
pub struct StateManager {
    current_state: SessionState,
}

impl StateManager {
    pub fn new() -> Self {
        Self {
            current_state: SessionState::LocalActive,
        }
    }

    pub fn state(&self) -> &SessionState {
        &self.current_state
    }

    pub fn is_local_active(&self) -> bool {
        matches!(self.current_state, SessionState::LocalActive)
    }

    pub fn is_controlling_remote(&self) -> bool {
        matches!(self.current_state, SessionState::ControllingRemote { .. })
    }

    pub fn is_remote_controlled(&self) -> bool {
        matches!(self.current_state, SessionState::RemoteControlled { .. })
    }

    /// Transition to controlling a remote peer
    pub fn start_controlling_remote(&mut self, target_peer_id: String) -> StateAction {
        self.current_state = SessionState::ControllingRemote {
            target_peer_id: target_peer_id.clone(),
        };
        StateAction::TrapAndSuppressLocal
    }

    /// Return from controlling remote peer back to local control
    pub fn return_to_local(&mut self) -> StateAction {
        match &self.current_state {
            SessionState::ControllingRemote { target_peer_id } => {
                let peer_id = target_peer_id.clone();
                self.current_state = SessionState::LocalActive;
                StateAction::ReleaseAndUnhideLocal
            }
            SessionState::RemoteControlled { .. } => {
                self.current_state = SessionState::LocalActive;
                StateAction::None
            }
            SessionState::LocalActive => StateAction::None,
        }
    }

    /// Set state to being controlled by a remote peer
    pub fn enter_remote_controlled(&mut self, controller_peer_id: String) -> StateAction {
        self.current_state = SessionState::RemoteControlled {
            controller_peer_id,
        };
        StateAction::None
    }

    /// User physically touched mouse/keyboard while being remotely controlled
    pub fn handle_physical_input_detected(&mut self) -> StateAction {
        if let SessionState::RemoteControlled { controller_peer_id } = &self.current_state {
            let peer = controller_peer_id.clone();
            self.current_state = SessionState::LocalActive;
            StateAction::NotifyPreempt { peer_id: peer }
        } else {
            StateAction::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_flow() {
        let mut mgr = StateManager::new();
        assert!(mgr.is_local_active());

        // Jump to remote
        let action = mgr.start_controlling_remote("peer-win".into());
        assert_eq!(action, StateAction::TrapAndSuppressLocal);
        assert!(mgr.is_controlling_remote());

        // Return to local
        let action = mgr.return_to_local();
        assert_eq!(action, StateAction::ReleaseAndUnhideLocal);
        assert!(mgr.is_local_active());
    }

    #[test]
    fn test_preemption_takeover() {
        let mut mgr = StateManager::new();
        mgr.enter_remote_controlled("peer-mac".into());
        assert!(mgr.is_remote_controlled());

        // Physical input detected on follower
        let action = mgr.handle_physical_input_detected();
        assert_eq!(action, StateAction::NotifyPreempt { peer_id: "peer-mac".into() });
        assert!(mgr.is_local_active());
    }
}
