# Only our native navigation/scroll controls; no hardware apply or startup action.
#requires -Version 5.1
param()
$ErrorActionPreference='Stop'
# UI labels below are German; pin the app language so the checks stay stable.
$env:OMEN_HUB_LANG = 'de'
$taskRepo=Split-Path -Parent $PSScriptRoot
$taskExe=Join-Path $taskRepo 'dist\Omen Hub++.exe'
$taskId='navigation-test-'+[Guid]::NewGuid().ToString('N')
$taskName='Navigation '+[Guid]::NewGuid().ToString('N')
$taskStore=& $taskExe profile-storage|Out-String|ConvertFrom-Json
$taskRoot=[IO.Path]::GetFullPath((Join-Path $taskStore.directory 'profiles'))
$taskFile=Join-Path $taskRoot ($taskId+'.json')
$taskProcess=$null;$taskCreated=$false
if(-not('OmenNavigationSmoke'-as[type])){Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class OmenNavigationSmoke {
 public delegate bool EnumWindow(IntPtr hwnd,IntPtr other);
 [StructLayout(LayoutKind.Sequential)] public struct Rect {public int left,top,right,bottom;}
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumWindow cb,IntPtr arg);
 [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr hwnd,EnumWindow cb,IntPtr arg);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint pid);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd,StringBuilder text,int size);
 [DllImport("user32.dll")] static extern int GetDlgCtrlID(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr hwnd,int id);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hwnd,int command);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd,out Rect rect);
 [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hwnd,out Rect rect);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd,uint message,UIntPtr value,IntPtr other);
 [DllImport("user32.dll",EntryPoint="SendMessageTimeoutW")] static extern IntPtr Send(IntPtr hwnd,uint message,UIntPtr value,IntPtr other,uint flags,uint timeout,out UIntPtr result);
 [DllImport("user32.dll",EntryPoint="SendMessageTimeoutW",CharSet=CharSet.Unicode)] static extern IntPtr Read(IntPtr hwnd,uint message,UIntPtr value,StringBuilder text,uint flags,uint timeout,out UIntPtr result);
 public static ulong Message(IntPtr hwnd,uint msg,ulong value=0,long other=0){UIntPtr result;if(Send(hwnd,msg,(UIntPtr)value,(IntPtr)other,2,1000,out result)==IntPtr.Zero)throw new Exception("Owned control did not answer");return result.ToUInt64();}
 public static string Text(IntPtr hwnd){var text=new StringBuilder(8192);UIntPtr result;Read(hwnd,13,(UIntPtr)text.Capacity,text,2,1000,out result);return text.ToString();}
 public static string Item(IntPtr hwnd,int index){var text=new StringBuilder(512);UIntPtr result;Read(hwnd,0x0148,(UIntPtr)index,text,2,1000,out result);return text.ToString();}
 public static IntPtr Find(uint pid,string expected){IntPtr found=IntPtr.Zero;EnumWindows((h,a)=>{uint owner;GetWindowThreadProcessId(h,out owner);var name=new StringBuilder(128);GetClassName(h,name,name.Capacity);if(owner==pid&&name.ToString()==expected){found=h;return false;}return true;},IntPtr.Zero);return found;}
 public static IntPtr Control(IntPtr hwnd,int id){var found=GetDlgItem(hwnd,id);if(found!=IntPtr.Zero)return found;EnumChildWindows(hwnd,(h,a)=>{if(GetDlgCtrlID(h)==id){found=h;return false;}return true;},IntPtr.Zero);return found;}
 public static int Top(IntPtr hwnd){Rect r;GetWindowRect(hwnd,out r);return r.top;}
 public static int Width(IntPtr hwnd){Rect r;GetClientRect(hwnd,out r);return r.right;}
 public static int Height(IntPtr hwnd){Rect r;GetClientRect(hwnd,out r);return r.bottom;}
}
'@
}
function Open-Navigation {param([string]$Command,[string]$Class)
 $script:taskProcess=if($Command){Start-Process -FilePath $taskExe -ArgumentList $Command -WindowStyle Hidden -PassThru}else{Start-Process -FilePath $taskExe -WindowStyle Hidden -PassThru}
 $taskUntil=(Get-Date).AddSeconds(12)
 do{$taskWindow=[OmenNavigationSmoke]::Find([uint32]$script:taskProcess.Id,$Class);if($taskWindow-ne[IntPtr]::Zero){return $taskWindow};Start-Sleep -Milliseconds 60}while((Get-Date)-lt$taskUntil)
 throw ('No owned '+$Class)
}
function Close-Navigation {param([IntPtr]$Window,[int]$Command=0)
 if($Command){[void][OmenNavigationSmoke]::PostMessage($Window,0x0111,[UIntPtr]::new([uint64]$Command),[IntPtr]::Zero)}else{[void][OmenNavigationSmoke]::PostMessage($Window,0x0010,[UIntPtr]::Zero,[IntPtr]::Zero)}
 if(-not$script:taskProcess.WaitForExit(4000)-or$script:taskProcess.ExitCode-ne0){throw 'Owned window did not close cleanly.'};$script:taskProcess.Dispose();$script:taskProcess=$null
}
$taskMeta=@('control-journal.json','startup.json')|ForEach-Object{Join-Path $taskStore.directory $_}
$taskBefore=@($taskMeta|ForEach-Object{if(Test-Path -LiteralPath $_){(Get-FileHash -LiteralPath $_).Hash}else{'absent'}})
try{
 if(Test-Path -LiteralPath $taskFile){throw 'Temporary fixture exists.'}
 & $taskExe profile-create $taskId $taskName|Out-Null;if($LASTEXITCODE-ne0){throw 'Cannot create metadata fixture.'};$taskCreated=$true
 $taskWindow=Open-Navigation '' 'OmenHubPlusPlusWindow'
 $taskUntil=(Get-Date).AddSeconds(12)
 do{$taskCombo=[OmenNavigationSmoke]::GetDlgItem($taskWindow,102);$taskCount=if($taskCombo-ne[IntPtr]::Zero){[OmenNavigationSmoke]::Message($taskCombo,0x0146)}else{0};if($taskCount-ge4){break};Start-Sleep -Milliseconds 80}while((Get-Date)-lt$taskUntil)
 $taskItems=@(for($taskI=0;$taskI-lt$taskCount;$taskI++){[OmenNavigationSmoke]::Item($taskCombo,$taskI)})
 if(($taskItems[0..3]-join',')-cne'Quiet (Default),Balanced (Default),Performance (Default),Unleashed (Default)'-or$taskItems-notcontains$taskName){throw 'Leistungsmodus defaults/custom entries are wrong.'}
 Close-Navigation $taskWindow 202
 foreach($taskMode in @('fan-ui','gpu-curve-ui')){
  $taskWindow=Open-Navigation $taskMode 'OmenHubPPCurveEditor'
  $taskUntil=(Get-Date).AddSeconds(12)
  do{$taskStatus=[OmenNavigationSmoke]::Text([OmenNavigationSmoke]::GetDlgItem($taskWindow,520));if($taskStatus-match'Entwurf'){break};Start-Sleep -Milliseconds 80}while((Get-Date)-lt$taskUntil)
  if($taskStatus-notmatch'Entwurf'){throw 'Curve load did not finish.'}
  [void][OmenNavigationSmoke]::ShowWindow($taskWindow,4)
  $taskVisible=@(505..507|ForEach-Object{[OmenNavigationSmoke]::IsWindowVisible([OmenNavigationSmoke]::GetDlgItem($taskWindow,$_))})
  $taskExpected=if($taskMode-eq'fan-ui'){'False,True,True'}else{'True,False,False'}
  if(($taskVisible-join',')-cne$taskExpected){throw ('Wrong curve scope '+$taskMode+': '+($taskVisible-join','))}
  Close-Navigation $taskWindow
 }
 $taskWindow=Open-Navigation 'presets-ui' 'OmenHubPPPresetEditor'
 $taskUntil=(Get-Date).AddSeconds(12)
 do{$taskStatus=[OmenNavigationSmoke]::Text([OmenNavigationSmoke]::GetDlgItem($taskWindow,711));if($taskStatus-match'Preset benennen'){break};Start-Sleep -Milliseconds 80}while((Get-Date)-lt$taskUntil)
 $taskPane=[OmenNavigationSmoke]::GetDlgItem($taskWindow,710);$taskHeader=[OmenNavigationSmoke]::Control($taskWindow,1200);$taskTop=[OmenNavigationSmoke]::Top($taskHeader)
 $taskCpuHeading=[OmenNavigationSmoke]::Control($taskWindow,721);$taskWindowsHeading=[OmenNavigationSmoke]::Control($taskWindow,722)
 if([OmenNavigationSmoke]::Text($taskCpuHeading)-cne'CPU'-or[OmenNavigationSmoke]::Text($taskWindowsHeading)-cne'Windows'-or[OmenNavigationSmoke]::Top($taskWindowsHeading)-le[OmenNavigationSmoke]::Top($taskCpuHeading)){throw 'Windows is not a separate section after CPU.'}
 [void][OmenNavigationSmoke]::Message($taskPane,0x0115,7)
 if([OmenNavigationSmoke]::Top($taskHeader)-ge$taskTop){throw 'Settings did not page to bottom.'}
 $taskWindowsTop=[OmenNavigationSmoke]::Top($taskWindowsHeading);$taskPaneTop=[OmenNavigationSmoke]::Top($taskPane)
 if($taskWindowsTop-lt$taskPaneTop-or$taskWindowsTop+32-gt$taskPaneTop+[OmenNavigationSmoke]::Height($taskPane)){throw 'Windows section heading is obscured at the bottom of the settings pane.'}
 [void][OmenNavigationSmoke]::Message($taskPane,0x0115,6)
 if([OmenNavigationSmoke]::Top($taskHeader)-ne$taskTop){throw 'Settings did not return to top.'}
 $taskX=[OmenNavigationSmoke]::Width($taskPane)-8
 [void][OmenNavigationSmoke]::Message($taskPane,0x0201,1,($taskX-bor(5-shl16)))
 [void][OmenNavigationSmoke]::Message($taskPane,0x0200,1,($taskX-bor(100-shl16)))
 [void][OmenNavigationSmoke]::Message($taskPane,0x0202,0,($taskX-bor(100-shl16)))
 if([OmenNavigationSmoke]::Top($taskHeader)-ge$taskTop){throw 'Dark thumb drag did not scroll settings.'}
 Close-Navigation $taskWindow
 $taskAfter=@($taskMeta|ForEach-Object{if(Test-Path -LiteralPath $_){(Get-FileHash -LiteralPath $_).Hash}else{'absent'}})
 if(($taskBefore-join',')-cne($taskAfter-join',')){throw 'Read-only navigation changed recovery/startup.'}
 [pscustomobject]@{measured_at=(Get-Date).ToString('o');binary_sha256=(Get-FileHash -LiteralPath $taskExe).Hash;defaults_and_custom_list=$true;scoped_gpu_fan_navigation=$true;settings_page_bounds=$true;dark_thumb_drag=$true;hardware_writes=$false;journal_startup_unchanged=$true;clean_exit=$true}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $taskRepo '.local\controls-navigation-smoke.json') -Encoding UTF8
 'Default/custom selector, scoped GPU/fan editors, settings scroll/pages/drag and unchanged journal/startup passed.'
}finally{
 if($taskProcess){$taskProcess.Refresh();if(-not$taskProcess.HasExited){Stop-Process -Id $taskProcess.Id};$taskProcess.Dispose()}
 if($taskCreated-and(Test-Path -LiteralPath $taskFile)){$taskResolved=(Resolve-Path -LiteralPath $taskFile).Path;if([IO.Path]::GetDirectoryName($taskResolved)-cne$taskRoot){throw 'Navigation cleanup escaped profiles.'};Remove-Item -LiteralPath $taskResolved}
}
