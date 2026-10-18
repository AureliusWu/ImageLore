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

$stage='LOAD_LEGACY_DACL_RESTORE'
# SetFileSecurity is obsolete; this test-only fallback restores a legacy DACL
# AutoInherited=false bit which Set-Acl normalizes. Never write owner or SACL.
# Compile before applying deny so a compiler failure cannot prevent restoration.
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class ImageLoreOwnedRawAclNative {
  [DllImport("advapi32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
  [return: MarshalAs(UnmanagedType.Bool)]
  private static extern bool SetFileSecurityW(string path, uint information, byte[] descriptor);
  public static bool RestoreLegacyDacl(string path, byte[] descriptor) {
    return SetFileSecurityW(path, 4, descriptor);
  }
}
'@

$access=[Security.AccessControl.AccessControlSections]::Access
$stage='CAPTURE_ORIGINAL_ACCESS'
$originalAcl=Get-Acl -LiteralPath $target
$originalAccess=$originalAcl.GetSecurityDescriptorSddlForm($access)
$originalBinary=$originalAcl.GetSecurityDescriptorBinaryForm()
$originalCanonical=$originalAcl.AreAccessRulesCanonical
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
  # Mark the original Access section for persistence without changing its SDDL.
  $originalAcl.SetSecurityDescriptorSddlForm($originalAccess,$access)
  Set-Acl -LiteralPath $target -AclObject $originalAcl
  $cleanupStage='VERIFY_RESTORED_ACCESS'
  $restored=Get-Acl -LiteralPath $target
  $originalRaw=[Security.AccessControl.RawSecurityDescriptor]::new($originalBinary,0)
  if($restored.GetSecurityDescriptorSddlForm($access) -cne $originalAccess -and
     -not ($originalRaw.ControlFlags -band [Security.AccessControl.ControlFlags]::DiscretionaryAclAutoInherited)){
    $cleanupStage='RESTORE_LEGACY_DACL'
    if(-not [ImageLoreOwnedRawAclNative]::RestoreLegacyDacl($target,$originalBinary)){
      throw [ComponentModel.Win32Exception]::new([Runtime.InteropServices.Marshal]::GetLastWin32Error())
    }
    $cleanupStage='VERIFY_RESTORED_ACCESS'
    $restored=Get-Acl -LiteralPath $target
  }
  if($restored.GetSecurityDescriptorSddlForm($access) -cne $originalAccess){
    try{
      # Descriptor and ACE encodings stay in memory. Only bounded scalars reach stderr.
      function Read-DaclDiagnostic([byte[]]$DescriptorBytes){
        $raw=[Security.AccessControl.RawSecurityDescriptor]::new($DescriptorBytes,0)
        $acl=$raw.DiscretionaryAcl
        $encoded=''
        $aces=@()
        $revision=-1
        if($null -ne $acl){
          $bytes=[byte[]]::new($acl.BinaryLength)
          $acl.GetBinaryForm($bytes,0)
          $encoded=[Convert]::ToBase64String($bytes)
          $revision=[int]$acl.Revision
          foreach($ace in $acl){
            $bytes=[byte[]]::new($ace.BinaryLength)
            $ace.GetBinaryForm($bytes,0)
            $aces+= [Convert]::ToBase64String($bytes)
          }
        }
        [pscustomobject]@{Flags=([int]$raw.ControlFlags -band 0x150C);IsNull=($null -eq $acl);Revision=$revision;Encoded=$encoded;Aces=$aces}
      }
      $before=Read-DaclDiagnostic $originalBinary
      $after=Read-DaclDiagnostic $restored.GetSecurityDescriptorBinaryForm()
      $diagnostic='OWNED_ACL_DIAGNOSTIC originalFlags={0} restoredFlags={1} flagsXor={2} daclBytesEqual={3} aceOrderEqual={4} aceMultisetEqual={5} originalAceCount={6} restoredAceCount={7} originalAclRevision={8} restoredAclRevision={9} originalCanonical={10} restoredCanonical={11} originalDaclNull={12} restoredDaclNull={13}' -f $before.Flags,$after.Flags,($before.Flags -bxor $after.Flags),($before.Encoded -ceq $after.Encoded),(($before.Aces -join '|') -ceq ($after.Aces -join '|')),((($before.Aces | Sort-Object) -join '|') -ceq (($after.Aces | Sort-Object) -join '|')),@($before.Aces).Count,@($after.Aces).Count,$before.Revision,$after.Revision,$originalCanonical,$restored.AreAccessRulesCanonical,$before.IsNull,$after.IsNull
      [Console]::Error.WriteLine($diagnostic)
    }catch{
      [Console]::Error.WriteLine(('OWNED_ACL_DIAGNOSTIC diagnostic-stage=CLASSIFY exception={0}' -f $_.Exception.GetType().Name))
    }
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
