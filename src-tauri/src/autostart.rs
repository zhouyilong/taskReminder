use std::ffi::OsStr;
use std::path::Path;

#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::path::PathBuf;

use crate::errors::AppError;

const AUTOSTART_LAUNCH_ARG: &str = "--autostart";

fn autostart_entry_name() -> &'static str {
    if crate::paths::is_dev_mode() {
        "TaskReminderApp-Dev"
    } else {
        "TaskReminderApp"
    }
}

pub fn launched_from_autostart() -> bool {
    std::env::args_os().any(|arg| arg == OsStr::new(AUTOSTART_LAUNCH_ARG))
}

fn current_executable_path() -> Result<std::path::PathBuf, AppError> {
    std::env::current_exe().map_err(|e| AppError::System(e.to_string()))
}

#[cfg(windows)]
fn windows_autostart_command(exe: &Path) -> String {
    format!("\"{}\" {}", exe.to_string_lossy(), AUTOSTART_LAUNCH_ARG)
}

#[cfg(target_os = "macos")]
fn macos_plist_content(exe: &Path) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n\
<dict>\n\
  <key>Label</key><string>{}</string>\n\
  <key>ProgramArguments</key>\n\
  <array><string>{}</string><string>{}</string></array>\n\
  <key>RunAtLoad</key><true/>\n\
</dict>\n\
</plist>\n",
        macos_launch_label(),
        exe.to_string_lossy(),
        AUTOSTART_LAUNCH_ARG
    )
}

#[cfg(target_os = "linux")]
fn linux_desktop_content(exe: &Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=TaskReminderApp\nExec={} {}\nX-GNOME-Autostart-enabled=true\n",
        exe.to_string_lossy(),
        AUTOSTART_LAUNCH_ARG
    )
}

pub fn enable_autostart() -> Result<(), AppError> {
    let exe = current_executable_path()?;

    #[cfg(windows)]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu
            .create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")
            .map_err(|e| AppError::System(e.to_string()))?;
        key.set_value(autostart_entry_name(), &windows_autostart_command(&exe))
            .map_err(|e| AppError::System(e.to_string()))?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        let plist_path = macos_plist_path()?;
        std::fs::create_dir_all(plist_path.parent().unwrap())?;
        std::fs::write(plist_path, macos_plist_content(&exe))?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        let desktop_path = linux_desktop_path()?;
        std::fs::create_dir_all(desktop_path.parent().unwrap())?;
        std::fs::write(desktop_path, linux_desktop_content(&exe))?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err(AppError::System("当前平台不支持开机自启".to_string()))
}

pub fn disable_autostart() -> Result<(), AppError> {
    #[cfg(windows)]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(key) = hkcu.open_subkey_with_flags(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            KEY_ALL_ACCESS,
        ) {
            let _ = key.delete_value(autostart_entry_name());
        }
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        let plist_path = macos_plist_path()?;
        if plist_path.exists() {
            std::fs::remove_file(plist_path)?;
        }
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        let desktop_path = linux_desktop_path()?;
        if desktop_path.exists() {
            std::fs::remove_file(desktop_path)?;
        }
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err(AppError::System("当前平台不支持开机自启".to_string()))
}

pub fn is_autostart_enabled() -> Result<bool, AppError> {
    #[cfg(windows)]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(key) = hkcu.open_subkey_with_flags(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            KEY_READ,
        ) {
            let value: Result<String, _> = key.get_value(autostart_entry_name());
            return Ok(value.is_ok());
        }
        return Ok(false);
    }

    #[cfg(target_os = "macos")]
    {
        let plist_path = macos_plist_path()?;
        return Ok(plist_path.exists());
    }

    #[cfg(target_os = "linux")]
    {
        let desktop_path = linux_desktop_path()?;
        return Ok(desktop_path.exists());
    }

    #[allow(unreachable_code)]
    Ok(false)
}

pub fn is_autostart_configuration_current() -> Result<bool, AppError> {
    let exe = current_executable_path()?;

    #[cfg(windows)]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(key) = hkcu.open_subkey_with_flags(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            KEY_READ,
        ) {
            let value: Result<String, _> = key.get_value(autostart_entry_name());
            return Ok(
                matches!(value, Ok(existing) if existing == windows_autostart_command(&exe)),
            );
        }
        return Ok(false);
    }

    #[cfg(target_os = "macos")]
    {
        let plist_path = macos_plist_path()?;
        if !plist_path.exists() {
            return Ok(false);
        }
        return Ok(std::fs::read_to_string(plist_path)? == macos_plist_content(&exe));
    }

    #[cfg(target_os = "linux")]
    {
        let desktop_path = linux_desktop_path()?;
        if !desktop_path.exists() {
            return Ok(false);
        }
        return Ok(std::fs::read_to_string(desktop_path)? == linux_desktop_content(&exe));
    }

    #[allow(unreachable_code)]
    Ok(false)
}

#[cfg(target_os = "macos")]
fn macos_plist_path() -> Result<PathBuf, AppError> {
    let home =
        dirs_next::home_dir().ok_or_else(|| AppError::System("无法获取用户目录".to_string()))?;
    Ok(home
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{}.plist", macos_launch_label())))
}

#[cfg(target_os = "macos")]
fn macos_launch_label() -> &'static str {
    if crate::paths::is_dev_mode() {
        "com.taskreminder.app.dev"
    } else {
        "com.taskreminder.app"
    }
}

#[cfg(target_os = "linux")]
fn linux_desktop_path() -> Result<PathBuf, AppError> {
    let home =
        dirs_next::home_dir().ok_or_else(|| AppError::System("无法获取用户目录".to_string()))?;
    Ok(home
        .join(".config")
        .join("autostart")
        .join(format!("{}.desktop", autostart_entry_name())))
}
