use std::collections::{BTreeMap, HashMap};

use ratatui::widgets::ScrollbarState;

use super::base_state::BaseState;

#[allow(non_camel_case_types)]
#[derive(Copy, Clone)]
pub enum UIScreen {
    CONFIG,
    TARGET_START,
    FINISHED,
}

#[allow(non_camel_case_types)]
#[derive(Copy, Clone)]
pub enum UITargetState {
    TARGET_START,
    TARGET_CHECKSUM,
    TARGET_UPLOADING,
    TARGET_NO_CHANGES,
    TARGET_FINISHING,
    TARGET_FINISHED,
}

pub struct TargetState {
    pub state: UITargetState,
    pub name: String,
    pub upload_package: String,
    pub upload_pos: u64,
    pub upload_len: u64,
}

pub struct UIStore {
    pub screen: UIScreen,
    pub targets_count: u32,
    pub packages_count: u32,
    pub deployed_count: u32,
    pub deployment_targets: BTreeMap<u32, TargetState>,

    // system
    pub vertical_scroll: u16,
    pub vertical_scroll_state: ScrollbarState,
}

impl BaseState<UIStore> for UIStore {
    fn new() -> UIStore {
        UIStore {
            screen: UIScreen::CONFIG,
            targets_count: 0,
            packages_count: 0,
            deployed_count: 0, // successfully deployed targets
            deployment_targets: BTreeMap::new(),
            vertical_scroll: 0,
            vertical_scroll_state: ScrollbarState::default(),
        }
    }
}

impl UIStore {
    pub fn set_screen(&mut self, state: UIScreen) -> &mut UIStore {
        self.screen = state;
        self
    }

    pub fn set_targets_count(&mut self, count: u32) -> &mut UIStore {
        self.targets_count = count;
        self
    }

    pub fn set_packages_count(&mut self, count: u32) -> &mut UIStore {
        self.packages_count = count;
        self
    }

    pub fn set_deployed_count(&mut self, count: u32) -> &mut UIStore {
        self.deployed_count = count;
        self
    }

    pub fn set_deployment_target(&mut self, index: u32, target: TargetState) -> &mut UIStore {
        self.deployment_targets.insert(index, target);
        self
    }

    pub fn finalize(&self) -> UIStore {
        UIStore {
            screen: self.screen,
            targets_count: self.targets_count,
            packages_count: self.packages_count,
            deployed_count: self.deployed_count,
            deployment_targets: BTreeMap::new(),
            vertical_scroll: 0,
            vertical_scroll_state: self.vertical_scroll_state,
        }
    }
}
