# Lecture d'une entrée générique du Gestionnaire d'identifiants Windows, partagée par
# release.ps1 (mot de passe de la clé de signature) et gitea-release.ps1 (jeton Gitea).
# À charger avec « . (Join-Path $PSScriptRoot 'credentials.ps1') ».

# Secret d'une entrée générique (CRED_TYPE_GENERIC), $null si elle n'existe pas.
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
