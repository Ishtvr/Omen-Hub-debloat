//! Task definition for "Start with Windows". The dashboard runs with
//! administrator rights, and Windows does not start such programs from the
//! Run key at sign-in, so a logon task with the highest run level starts it.

/// Only these dashboard arguments may be stored in the task.
pub const ARGUMENTS: [&str; 2] = ["", "--tray"];

/// Task name for the signed-in user.
pub fn task_name(sid: &str) -> String {
    format!("OmenHubPP-Tray-{sid}")
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

/// Restarts after a failed exit (a crash): the automatic battery change and
/// "Keep preset" live in this process.
pub const RESTART_COUNT: u32 = 3;
/// Shortest interval Task Scheduler accepts.
pub const RESTART_INTERVAL: &str = "PT1M";

/// Starts `executable` with `arguments` (one of [`ARGUMENTS`]) at the user's
/// sign-in with the highest run level, on battery too, without a time limit
/// and without a second copy. An exit with an error code (a crash) starts it
/// again after a minute, up to [`RESTART_COUNT`] times; a normal exit does not.
pub fn task_xml(sid: &str, executable: &str, arguments: &str) -> String {
    let (sid, executable, arguments) = (escape(sid), escape(executable), escape(arguments));
    format!(
        r#"<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{sid}</UserId>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{sid}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <RestartOnFailure>
      <Interval>{RESTART_INTERVAL}</Interval>
      <Count>{RESTART_COUNT}</Count>
    </RestartOnFailure>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>5</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>"{executable}"</Command>
      <Arguments>{arguments}</Arguments>
    </Exec>
  </Actions>
</Task>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logon_task_starts_the_tray_elevated_on_battery_without_time_limit() {
        let xml = task_xml(
            "S-1-5-21-1",
            r"D:\Omen Gaming Hub Rewrite\dist\Omen Hub++.exe",
            "--tray",
        );
        for expected in [
            "<UserId>S-1-5-21-1</UserId>",
            "<RunLevel>HighestAvailable</RunLevel>",
            "<LogonType>InteractiveToken</LogonType>",
            "<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>",
            "<StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>",
            "<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>",
            "<MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>",
            "<RestartOnFailure>",
            "<Interval>PT1M</Interval>",
            "<Count>3</Count>",
            r#"<Command>"D:\Omen Gaming Hub Rewrite\dist\Omen Hub++.exe"</Command>"#,
            "<Arguments>--tray</Arguments>",
        ] {
            assert!(xml.contains(expected), "{expected}");
        }
        assert_eq!(xml.matches("<UserId>").count(), 2);
        assert_eq!(task_name("S-1-5-21-1"), "OmenHubPP-Tray-S-1-5-21-1");
    }

    #[test]
    fn paths_cannot_inject_task_markup() {
        let xml = task_xml("S-1", r#"C:\A&B\<x>"'.exe"#, "");
        assert!(xml.contains(r#"<Command>"C:\A&amp;B\&lt;x&gt;&quot;&apos;.exe"</Command>"#));
        assert!(!xml.contains("<x>"));
        assert!(xml.contains("<Arguments></Arguments>"));
    }
}
