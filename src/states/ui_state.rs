use super::base_state::BaseState;

#[allow(non_camel_case_types)]
#[derive(Copy, Clone)]
pub enum UIScreen {
    CONFIG,
    TARGET_START,
}

pub struct UIStore {
    pub screen: UIScreen,
    pub targets_count: u32,
    pub packages_count: u32,
    pub deployment_target: String,
}

impl BaseState<UIStore> for UIStore {
    fn new() -> UIStore {
        UIStore {
            screen: UIScreen::CONFIG,
            targets_count: 0,
            packages_count: 0,
            deployment_target: String::from(""),
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

    pub fn set_deployment_target(&mut self, target: String) -> &mut UIStore {
        self.deployment_target = target;
        self
    }

    pub fn finalize(&self) -> UIStore {
        UIStore {
            screen: self.screen,
            targets_count: self.targets_count,
            packages_count: self.packages_count,
            deployment_target: self.deployment_target.to_owned(),
        }
    }
}
