param([Parameter(Mandatory=$true)][string]$OwnedRoot)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$stage='LOAD_SECURITY_MODULE'
$cleanupStage='NOT_STARTED'
try{
# A parent pwsh process can expose PowerShell 7 modules to Windows PowerShell.
# Load this host's matching ACL cmdlets without changing the inherited environment.
Import-Module (Join-Path $PSHOME 'Modules\Microsoft.PowerShell.Security\Microsoft.PowerShell.Security.psd1') -ErrorAction Stop

$stage='VALIDATE_OWNED_ROOT'
$root=[IO.Path]::GetFullPath($OwnedRoot)
if(-not [IO.Path]::IsPathRooted($OwnedRoot) -or -not [IO.Path]::GetFileName($root).StartsWith('imagelore-raw-acl-preservation-',[StringComparison]::Ordinal)){
  throw 'Only the uniquely owned raw ACL test root is allowed'
}
$target=Join-Path $root 'backups'
$stage='VALIDATE_DIRECTORIES'
foreach($item in @((Get-Item -LiteralPath $root),(Get-Item -LiteralPath $target))){
  if(-not $item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)){
    throw 'ACL test directories must be ordinary directories'
  }
}
$stage='VALIDATE_MARKER'
if([IO.File]::ReadAllText((Join-Path $root '.imagelore-raw-acl-test')) -cne "ImageLore raw ACL test v1`n"){
  throw 'Exact owned ACL test marker is missing'
}

$access=[Security.AccessControl.AccessControlSections]::Access
$stage='CAPTURE_ORIGINAL_ACCESS'
$originalAcl=Get-Acl -LiteralPath $target
$originalAccess=$originalAcl.GetSecurityDescriptorSddlForm($access)
$workingAcl=Get-Acl -LiteralPath $target
$stage='PREPARE_DENY_RULE'
$sid=[Security.Principal.WindowsIdentity]::GetCurrent().User
$deny=[Security.AccessControl.FileSystemAccessRule]::new($sid,[Security.AccessControl.FileSystemRights]::CreateFiles,[Security.AccessControl.InheritanceFlags]::None,[Security.AccessControl.PropagationFlags]::None,[Security.AccessControl.AccessControlType]::Deny)
$null=$workingAcl.AddAccessRule($deny)
$stage='VERIFY_ORIGINAL_UNCHANGED'
if($originalAcl.GetSecurityDescriptorSddlForm($access) -cne $originalAccess){
  throw 'Original access ACL must remain untouched'
}

try{
  $stage='APPLY_DENY'
  Set-Acl -LiteralPath $target -AclObject $workingAcl
  $stage='WRITE_READY'
  [Console]::Out.WriteLine('ACL_DENY_READY')
  [Console]::Out.Flush()
  $stage='WAIT_RESTORE_REQUEST'
  if([Console]::ReadLine() -cne 'RESTORE'){
    throw 'ACL controller ended without an explicit restore request'
  }
}finally{
  # EOF after a controller failure also restores the independently retained ACL.
  $cleanupStage='RESTORE_ACCESS'
  Set-Acl -LiteralPath $target -AclObject $originalAcl
  $cleanupStage='VERIFY_RESTORED_ACCESS'
  $restored=Get-Acl -LiteralPath $target
  if($restored.GetSecurityDescriptorSddlForm($access) -cne $originalAccess){
    throw 'Original access ACL was not restored exactly'
  }
  $cleanupStage='WRITE_RESTORED'
  [Console]::Out.WriteLine('ACL_ACCESS_RESTORED')
  [Console]::Out.Flush()
}
}catch{
  # Keep CI diagnostics useful without exposing ACL descriptors, identities or environment.
  [Console]::Error.WriteLine(('OWNED_ACL_FAILURE stage={0} cleanup-stage={1} exception={2}' -f $stage,$cleanupStage,$_.Exception.GetType().Name))
  exit 1
}
