#[cfg(target_os = "macos")]
mod imp {
    use std::process::Command;

    use anyhow::{bail, Result};

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
    }

    /// Read-only check — does not prompt the user.
    pub fn has_screen_recording() -> bool {
        unsafe { CGPreflightScreenCaptureAccess() }
    }

    pub fn check_screen_recording() -> Result<()> {
        if has_screen_recording() {
            return Ok(());
        }
        // Triggers the macOS permission prompt and adds nabu to the list.
        unsafe { CGRequestScreenCaptureAccess() };
        // Also open the exact pane as a fallback in case the prompt was
        // already dismissed in this session.
        open_screen_recording_settings();
        bail!(
            "Screen Recording permission required for system audio capture.\n\
             \n  → System Settings has been opened to Privacy & Security → Screen Recording.\n\
             \n  1. Toggle your terminal app ON in the list.\n  \
                2. macOS will ask you to quit and reopen the terminal — do that.\n  \
                3. Start nabu again from the new terminal window.\n\
             \n  (macOS caches the permission decision until the terminal process restarts.)"
        )
    }

    pub fn open_screen_recording_settings() {
        let _ = Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
            .spawn();
    }

    pub fn open_microphone_settings() {
        let _ = Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
            .spawn();
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use anyhow::Result;
    pub fn has_screen_recording() -> bool {
        true
    }
    pub fn check_screen_recording() -> Result<()> {
        Ok(())
    }
    pub fn open_screen_recording_settings() {}
    pub fn open_microphone_settings() {}
}

pub use imp::{check_screen_recording, has_screen_recording, open_microphone_settings};
