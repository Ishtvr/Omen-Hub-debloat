//! Task definition for "Start with Windows". The dashboard runs with
//! administrator rights, and Windows does not start such programs from the
//! Run key at sign-in, so a logon task with the highest run level starts it.

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

/// Starts `executable --tray` at the user's sign-in with the highest run
/// level, on battery too, without a time limit and without a second copy.
pub fn task_xml(sid: &str, executable: &str) -> String {
    let (sid, executable) = (escape(sid), escape(executable));
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
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>5</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>"{executable}"</Command>
      <Arguments>--tray</Arguments>
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
        );
        for expected in [
            "<UserId>S-1-5-21-1</UserId>",
            "<RunLevel>HighestAvailable</RunLevel>",
            "<LogonType>InteractiveToken</LogonType>",
            "<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>",
            "<StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>",
            "<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>",
            "<MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>",
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
        let xml = task_xml("S-1", r#"C:\A&B\<x>"'.exe"#);
        assert!(xml.contains(r#"<Command>"C:\A&amp;B\&lt;x&gt;&quot;&apos;.exe"</Command>"#));
        assert!(!xml.contains("<x>"));
    }
}
