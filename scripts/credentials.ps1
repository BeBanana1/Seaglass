# Reads a generic entry of the Windows Credential Manager, shared by release.ps1 (signing
# key password) and github-release.ps1 (GitHub token).
# Load with ". (Join-Path $PSScriptRoot 'credentials.ps1')".

# Secret of a generic entry (CRED_TYPE_GENERIC), $null if it does not exist.
function Read-StoredSecret([string]$target) {
    if (-not ('Seaglass.StoredCredential' -as [type])) {
        Add-Type -Namespace Seaglass -Name StoredCredential -MemberDefinition @'
[StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
private struct CREDENTIAL {
    public int Flags; public int Type; public string TargetName; public string Comment;
    public long LastWritten; public int CredentialBlobSize; public IntPtr CredentialBlob;
    public int Persist; public int AttributeCount; public IntPtr Attributes;
    public string TargetAlias; public string UserName;
}
[DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
private static extern bool CredReadW(string target, int type, int flags, out IntPtr credential);
[DllImport("advapi32.dll")]
private static extern void CredFree(IntPtr credential);
public static string Read(string target) {
    IntPtr ptr;
    if (!CredReadW(target, 1, 0, out ptr)) return null;
    try {
        var c = (CREDENTIAL)Marshal.PtrToStructure(ptr, typeof(CREDENTIAL));
        return c.CredentialBlobSize == 0 ? "" : Marshal.PtrToStringUni(c.CredentialBlob, c.CredentialBlobSize / 2);
    } finally { CredFree(ptr); }
}
'@
    }
    [Seaglass.StoredCredential]::Read($target)
}
