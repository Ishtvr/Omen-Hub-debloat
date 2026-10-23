# Native tray lifecycle regression. Operates only on this script's own windows
# and notification icons; never restarts Explorer or sends hardware commands.
#requires -Version 5.1
param([string]$Executable='')
$ErrorActionPreference='Stop'
$taskRepo=Split-Path -Parent $PSScriptRoot
if(-not$Executable){$Executable=Join-Path $taskRepo 'dist\Omen Hub++.exe'}
$taskExe=(Resolve-Path -LiteralPath $Executable).Path
$taskOut=Join-Path $taskRepo '.local'
New-Item -ItemType Directory -Path $taskOut -Force|Out-Null
$taskSettings=Join-Path ([Environment]::GetFolderPath('UserProfile')) '.omen-hub-pp\app-settings.json'
$taskOriginalSettings=if(Test-Path -LiteralPath $taskSettings){[IO.File]::ReadAllBytes($taskSettings)}else{$null}
if($null-ne$taskOriginalSettings){[IO.File]::WriteAllBytes((Join-Path $taskOut 'tray-settings-before.bin'),$taskOriginalSettings)}
$taskSettingsWritten=$null
$taskProcess=$null
$taskRecord=[ordered]@{started_at=(Get-Date).ToString('o');binary_sha256=(Get-FileHash -LiteralPath $taskExe).Hash;error=$null;cases=@()}
if(-not('OmenTraySmoke'-as[type])){Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class OmenTraySmoke {
 public delegate bool EnumWindow(IntPtr hwnd,IntPtr arg);
 [StructLayout(LayoutKind.Sequential)] public struct Rect {public int Left,Top,Right,Bottom;}
 [StructLayout(LayoutKind.Sequential)] public struct Identifier {public uint Size;public IntPtr Window;public uint Id;public Guid Guid;}
 [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)] public struct IconData {
  public uint Size;public IntPtr Window;public uint Id,Flags,Callback;public IntPtr Icon;
  [MarshalAs(UnmanagedType.ByValTStr,SizeConst=128)] public string Tip;
  public uint State,StateMask;
  [MarshalAs(UnmanagedType.ByValTStr,SizeConst=256)] public string Info;
  public uint Version;
  [MarshalAs(UnmanagedType.ByValTStr,SizeConst=64)] public string Title;
  public uint InfoFlags;public Guid Guid;public IntPtr Balloon;
 }
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumWindow cb,IntPtr arg);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint owner);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd,StringBuilder value,int length);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd,uint msg,UIntPtr wp,IntPtr lp);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hwnd,int cmd);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr hwnd,int id);
 [DllImport("user32.dll")] public static extern uint IsDlgButtonChecked(IntPtr hwnd,int id);
 [DllImport("user32.dll",EntryPoint="SendMessageTimeoutW")] static extern IntPtr Send(IntPtr hwnd,uint msg,UIntPtr wp,IntPtr lp,uint flags,uint timeout,out UIntPtr result);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern uint RegisterWindowMessage(string value);
 [DllImport("shell32.dll")] static extern int Shell_NotifyIconGetRect(ref Identifier id,out Rect rect);
 [DllImport("shell32.dll",CharSet=CharSet.Unicode)] static extern bool Shell_NotifyIcon(uint message,ref IconData data);
 public static IntPtr Find(uint pid){IntPtr result=IntPtr.Zero;EnumWindows((h,a)=>{uint owner;GetWindowThreadProcessId(h,out owner);var name=new StringBuilder(128);GetClassName(h,name,name.Capacity);if(owner==pid&&name.ToString()=="OmenHubPlusPlusWindow"){result=h;return false;}return true;},IntPtr.Zero);return result;}
 public static bool HasIcon(IntPtr hwnd){var id=new Identifier{Size=(uint)Marshal.SizeOf(typeof(Identifier)),Window=hwnd,Id=1};Rect rect;return Shell_NotifyIconGetRect(ref id,out rect)==0&&rect.Right>rect.Left&&rect.Bottom>rect.Top;}
 public static bool DeleteOwnedIcon(IntPtr hwnd){var data=new IconData{Size=(uint)Marshal.SizeOf(typeof(IconData)),Window=hwnd,Id=1,Tip="",Info="",Title=""};return Shell_NotifyIcon(2,ref data);}
 public static void Check(IntPtr checkbox,bool value){UIntPtr result;if(Send(checkbox,0x00F1,(UIntPtr)(value?1:0),IntPtr.Zero,2,1000,out result)==IntPtr.Zero)throw new Exception("Checkbox did not respond");}
 public static void TrayEvent(IntPtr hwnd,uint message,bool version4){uint packed=message|(version4?0x00010000u:0);PostMessage(hwnd,0x8002,UIntPtr.Zero,(IntPtr)(long)packed);}
}
'@}
function Wait-For {param([scriptblock]$Condition,[string]$Failure,[int]$Seconds=5);$taskDeadline=(Get-Date).AddSeconds($Seconds);do{if(& $Condition){return};Start-Sleep -Milliseconds 80}while((Get-Date)-lt$taskDeadline);throw $Failure}
function Record {param([string]$Case);$taskRecord.cases+=@($Case)}
function Set-CloseToTray {param([IntPtr]$Window,[bool]$Enabled)
 $taskBox=[OmenTraySmoke]::GetDlgItem($Window,191)
 if($taskBox-eq[IntPtr]::Zero){throw 'Close-to-tray checkbox missing.'}
 [OmenTraySmoke]::Check($taskBox,$Enabled)
 [void][OmenTraySmoke]::PostMessage($Window,0x0111,[UIntPtr]::new([uint64]191),$taskBox)
 Wait-For {if(-not(Test-Path -LiteralPath $taskSettings)){return $false};(Get-Content -LiteralPath $taskSettings -Raw|ConvertFrom-Json).close_to_tray-eq$Enabled} 'App preference was not saved.'
 $script:taskSettingsWritten=(Get-FileHash -LiteralPath $taskSettings).Hash
}
try{
 if([IntPtr]::Size-ne8){throw 'This qualification requires 64-bit PowerShell.'}
 $taskProcess=Start-Process -FilePath $taskExe -ArgumentList '--tray' -WindowStyle Hidden -PassThru
 $null=$taskProcess.Handle
 $taskWindow=[IntPtr]::Zero
 Wait-For {$script:taskWindow=[OmenTraySmoke]::Find([uint32]$taskProcess.Id);$script:taskWindow-ne[IntPtr]::Zero-and[OmenTraySmoke]::HasIcon($script:taskWindow)} 'Hidden startup has no registered tray icon.' 12
 if([OmenTraySmoke]::IsWindowVisible($taskWindow)){throw 'Tray startup showed a window.'}
 Record 'hidden-startup-icon'

 # Duplicate delivery must not turn a successful NIM_ADD into a false failure.
 $taskTaskbar=[OmenTraySmoke]::RegisterWindowMessage('TaskbarCreated')
 [void][OmenTraySmoke]::PostMessage($taskWindow,$taskTaskbar,[UIntPtr]::Zero,[IntPtr]::Zero)
 Start-Sleep -Milliseconds 250
 if([OmenTraySmoke]::IsWindowVisible($taskWindow)-or-not[OmenTraySmoke]::HasIcon($taskWindow)){throw 'Duplicate TaskbarCreated lost the hidden tray state.'}
 Record 'duplicate-taskbar-notification'

 # The failure/recovery stimulus is limited to this test-owned icon.
 if(-not[OmenTraySmoke]::DeleteOwnedIcon($taskWindow)){throw 'Could not remove the owned test icon.'}
 Wait-For {-not[OmenTraySmoke]::HasIcon($taskWindow)} 'Owned icon was not removed.'
 [void][OmenTraySmoke]::PostMessage($taskWindow,$taskTaskbar,[UIntPtr]::Zero,[IntPtr]::Zero)
 Wait-For {[OmenTraySmoke]::HasIcon($taskWindow)} 'Taskbar recovery did not register the icon.'
 if([OmenTraySmoke]::IsWindowVisible($taskWindow)){throw 'Taskbar recovery unexpectedly opened the hidden app.'}
 Record 'lost-icon-taskbar-recovery'

 [OmenTraySmoke]::TrayEvent($taskWindow,0x0202,$false)
 Wait-For {[OmenTraySmoke]::IsWindowVisible($taskWindow)-and[OmenTraySmoke]::GetDlgItem($taskWindow,103)-ne[IntPtr]::Zero} 'Tray click did not open usable controls.'
 if([OmenTraySmoke]::IsDlgButtonChecked($taskWindow,191)-ne1){Set-CloseToTray $taskWindow $true}
 Record 'tray-click-open'

 if(-not[OmenTraySmoke]::DeleteOwnedIcon($taskWindow)){throw 'Could not remove owned icon before minimize.'}
 [void][OmenTraySmoke]::ShowWindow($taskWindow,6)
 Wait-For {-not[OmenTraySmoke]::IsWindowVisible($taskWindow)-and[OmenTraySmoke]::HasIcon($taskWindow)} 'Minimize hid the app without repairing its tray icon.'
 if([OmenTraySmoke]::GetDlgItem($taskWindow,103)-ne[IntPtr]::Zero){throw 'Minimize did not release dashboard controls.'}
 Record 'minimize-repairs-icon'

 [OmenTraySmoke]::TrayEvent($taskWindow,0x0401,$true)
 Wait-For {[OmenTraySmoke]::IsWindowVisible($taskWindow)-and[OmenTraySmoke]::GetDlgItem($taskWindow,103)-ne[IntPtr]::Zero} 'Keyboard tray activation did not rebuild controls.'
 Record 'keyboard-tray-open'
 if(-not[OmenTraySmoke]::DeleteOwnedIcon($taskWindow)){throw 'Could not remove owned icon before close.'}
 [void][OmenTraySmoke]::PostMessage($taskWindow,0x0010,[UIntPtr]::Zero,[IntPtr]::Zero)
 Wait-For {-not[OmenTraySmoke]::IsWindowVisible($taskWindow)-and[OmenTraySmoke]::HasIcon($taskWindow)} 'Close hid the app without repairing its tray icon.'
 $taskProcess.Refresh();if($taskProcess.HasExited){throw 'Close exited instead of keeping the tray app alive.'}
 Record 'close-repairs-icon'

 if(-not[OmenTraySmoke]::DeleteOwnedIcon($taskWindow)){throw 'Could not remove the owned icon after close.'}
 [void][OmenTraySmoke]::PostMessage($taskWindow,$taskTaskbar,[UIntPtr]::Zero,[IntPtr]::Zero)
 Wait-For {[OmenTraySmoke]::HasIcon($taskWindow)} 'Recovery after close did not restore the icon.'
 if([OmenTraySmoke]::IsWindowVisible($taskWindow)-or[OmenTraySmoke]::GetDlgItem($taskWindow,103)-ne[IntPtr]::Zero){throw 'Hidden recovery changed window visibility or rebuilt hidden controls.'}
 Record 'lost-icon-after-close'

 [void][OmenTraySmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]202),[IntPtr]::Zero)
 if(-not$taskProcess.WaitForExit(4000)){throw 'Tray Exit did not end the owned app.'}
 Wait-For {-not[OmenTraySmoke]::HasIcon($taskWindow)} 'Exit left an orphan tray icon.'
 Record 'exit-removes-icon'
 $taskProcess.Dispose();$taskProcess=$null

 $taskProcess=Start-Process -FilePath $taskExe -WindowStyle Normal -PassThru
 $null=$taskProcess.Handle
 Wait-For {$script:taskWindow=[OmenTraySmoke]::Find([uint32]$taskProcess.Id);$script:taskWindow-ne[IntPtr]::Zero-and[OmenTraySmoke]::IsWindowVisible($script:taskWindow)-and[OmenTraySmoke]::HasIcon($script:taskWindow)-and[OmenTraySmoke]::GetDlgItem($script:taskWindow,191)-ne[IntPtr]::Zero} 'Normal startup did not create its window and icon.' 12
 Record 'normal-startup-icon'
 Set-CloseToTray $taskWindow $false
 [void][OmenTraySmoke]::PostMessage($taskWindow,0x0010,[UIntPtr]::Zero,[IntPtr]::Zero)
 if(-not$taskProcess.WaitForExit(4000)){throw 'Close-to-tray disabled did not exit.'}
 Wait-For {-not[OmenTraySmoke]::HasIcon($taskWindow)} 'Disabled close-to-tray left an icon after exit.'
 Record 'close-preference-off-exits'
}catch{$taskRecord.error=$_.Exception.Message}
finally{
 if($taskProcess){$taskProcess.Refresh();if(-not$taskProcess.HasExited){$taskOwned=[OmenTraySmoke]::Find([uint32]$taskProcess.Id);if($taskOwned-ne[IntPtr]::Zero){[void][OmenTraySmoke]::PostMessage($taskOwned,0x0111,[UIntPtr]::new([uint64]202),[IntPtr]::Zero)};if(-not$taskProcess.WaitForExit(4000)){Stop-Process -Id $taskProcess.Id}};$taskProcess.Dispose()}
 if($taskSettingsWritten){
  if(-not(Test-Path -LiteralPath $taskSettings)-or(Get-FileHash -LiteralPath $taskSettings).Hash-cne$taskSettingsWritten){$taskRecord.error='App preference changed externally; original backup was retained and the file was not overwritten.'}
  elseif($null-eq$taskOriginalSettings){Remove-Item -LiteralPath $taskSettings}
  else{[IO.File]::WriteAllBytes($taskSettings,$taskOriginalSettings)}
 }
 $taskRecord.preference_restored=if($null-eq$taskOriginalSettings){-not(Test-Path -LiteralPath $taskSettings)}else{(Test-Path -LiteralPath $taskSettings)-and[Convert]::ToBase64String([IO.File]::ReadAllBytes($taskSettings))-ceq[Convert]::ToBase64String($taskOriginalSettings)}
 if(-not$taskRecord.preference_restored-and-not$taskRecord.error){$taskRecord.error='Original close-to-tray preference was not restored.'}
 $taskRecord.finished_at=(Get-Date).ToString('o')
 $taskRecord|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $taskOut 'tray-ui-smoke.json') -Encoding UTF8
}
if($taskRecord.error){throw $taskRecord.error}
'Native tray startup, minimize/close recovery, keyboard activation and clean exit passed.'
