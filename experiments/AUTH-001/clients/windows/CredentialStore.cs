using System;
using System.Collections.Generic;
using System.IO;
using System.Security.AccessControl;
using System.Security.Cryptography;
using System.Security.Principal;
using System.Text;

namespace Kehila.Auth001.Client
{
    internal sealed class Credential : IDisposable
    {
        internal readonly char[] Token;
        internal readonly bool Retiring;
        internal Credential(char[] token, bool retiring) { Token = token; Retiring = retiring; }
        public void Dispose() { Array.Clear(Token, 0, Token.Length); }
    }

    // Fixed two-file cache: no password, plaintext fallback, random orphan-file
    // accumulation, or selection of a "newest" interrupted write.
    internal sealed class CredentialStore
    {
        private const int Limit = 4096;
        private readonly string directory;
        private readonly SecurityIdentifier owner;
        private readonly HashSet<string> trustedOwners;
        private readonly string current;
        private readonly string pending;
        private static readonly byte[] Entropy = Encoding.ASCII.GetBytes("kehila/auth001/cli/v1");

        internal CredentialStore(string directory)
        {
            this.directory = Path.GetFullPath(directory);
            using (WindowsIdentity identity = WindowsIdentity.GetCurrent()) { owner = identity.User; }
            trustedOwners = new HashSet<string>(StringComparer.Ordinal);
            trustedOwners.Add(owner.Value);
            trustedOwners.Add(new SecurityIdentifier(WellKnownSidType.LocalSystemSid, null).Value);
            trustedOwners.Add(new SecurityIdentifier(WellKnownSidType.BuiltinAdministratorsSid, null).Value);
            trustedOwners.Add(((SecurityIdentifier)new NTAccount("NT SERVICE", "TrustedInstaller").Translate(typeof(SecurityIdentifier))).Value);
            current = Path.Combine(this.directory, "credential.dat");
            pending = Path.Combine(this.directory, "credential.pending");
            string parent = Path.GetDirectoryName(this.directory);
            VerifyChain(parent);
            if (!Directory.Exists(this.directory)) { Directory.CreateDirectory(this.directory, DirectorySecurity()); }
            VerifyPrivateDirectory();
        }

        internal static CredentialStore ForCurrentUser()
        {
            return new CredentialStore(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Kehila.Auth001"));
        }

        private DirectorySecurity DirectorySecurity()
        {
            DirectorySecurity security = new DirectorySecurity();
            security.SetAccessRuleProtection(true, false); security.SetOwner(owner);
            security.AddAccessRule(new FileSystemAccessRule(owner, FileSystemRights.FullControl,
                InheritanceFlags.ContainerInherit | InheritanceFlags.ObjectInherit, PropagationFlags.None, AccessControlType.Allow));
            return security;
        }

        private FileSecurity FileSecurity()
        {
            FileSecurity security = new FileSecurity();
            security.SetAccessRuleProtection(true, false); security.SetOwner(owner);
            security.AddAccessRule(new FileSystemAccessRule(owner, FileSystemRights.FullControl, AccessControlType.Allow));
            return security;
        }

        private static void Require(bool condition) { if (!condition) { throw new IOException("Credential protection check failed."); } }

        private static bool Present(string path)
        {
            try { File.GetAttributes(path); return true; }
            catch (FileNotFoundException) { return false; }
        }

        private void VerifyChain(string path)
        {
            string full = Path.GetFullPath(path);
            string root = Path.GetPathRoot(full);
            Require(root.Length == 3 && root[1] == ':' && !full.StartsWith(@"\\", StringComparison.Ordinal));
            string cursor = root;
            VerifyAncestor(cursor);
            foreach (string part in full.Substring(root.Length).Split(Path.DirectorySeparatorChar))
            {
                if (part.Length == 0) { continue; }
                cursor = Path.Combine(cursor, part); VerifyAncestor(cursor);
            }
        }

        private void VerifyAncestor(string path)
        {
            FileAttributes attributes = File.GetAttributes(path);
            Require((attributes & FileAttributes.Directory) != 0 && (attributes & FileAttributes.ReparsePoint) == 0);
            DirectorySecurity security = Directory.GetAccessControl(path, AccessControlSections.Owner | AccessControlSections.Access);
            Require(trustedOwners.Contains(((SecurityIdentifier)security.GetOwner(typeof(SecurityIdentifier))).Value));
            const FileSystemRights dangerous = FileSystemRights.WriteData | FileSystemRights.WriteExtendedAttributes |
                FileSystemRights.WriteAttributes | FileSystemRights.Delete | FileSystemRights.DeleteSubdirectoriesAndFiles |
                FileSystemRights.ChangePermissions | FileSystemRights.TakeOwnership;
            foreach (FileSystemAccessRule rule in security.GetAccessRules(true, true, typeof(SecurityIdentifier)))
            {
                if ((rule.PropagationFlags & PropagationFlags.InheritOnly) != 0 || rule.AccessControlType != AccessControlType.Allow) { continue; }
                Require(trustedOwners.Contains(rule.IdentityReference.Value) || (rule.FileSystemRights & dangerous) == 0);
            }
        }

        private void VerifyPrivate(FileSystemSecurity security)
        {
            Require(security.AreAccessRulesProtected && owner.Equals(security.GetOwner(typeof(SecurityIdentifier))));
            int grants = 0;
            foreach (FileSystemAccessRule rule in security.GetAccessRules(true, true, typeof(SecurityIdentifier)))
            {
                Require(rule.AccessControlType == AccessControlType.Allow && owner.Equals(rule.IdentityReference) &&
                    rule.FileSystemRights == FileSystemRights.FullControl);
                grants++;
            }
            Require(grants == 1);
        }

        private void VerifyPrivateDirectory()
        {
            VerifyChain(directory);
            VerifyPrivate(Directory.GetAccessControl(directory, AccessControlSections.Owner | AccessControlSections.Access));
        }

        private FileStream OpenChecked(string path, bool allowEmpty = false)
        {
            VerifyPrivateDirectory();
            FileAttributes attributes = File.GetAttributes(path);
            Require((attributes & (FileAttributes.Directory | FileAttributes.ReparsePoint | FileAttributes.Device)) == 0);
            FileStream file = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.None, Limit, FileOptions.SequentialScan);
            try
            {
                VerifyPrivate(file.GetAccessControl());
                Require(file.CanSeek && (allowEmpty || file.Length > 0) && file.Length <= Limit);
                Require((File.GetAttributes(path) & FileAttributes.ReparsePoint) == 0);
                VerifyPrivateDirectory();
                return file;
            }
            catch { file.Dispose(); throw; }
        }

        private void RequireSettled()
        {
            VerifyPrivateDirectory();
            Require(!Present(pending));
        }

        internal Credential Read(string origin, byte[] root)
        {
            RequireSettled();
            if (!Present(current)) { return null; }
            byte[] encrypted;
            using (FileStream file = OpenChecked(current))
            {
                encrypted = new byte[(int)file.Length]; int filled = 0;
                while (filled < encrypted.Length)
                {
                    int count = file.Read(encrypted, filled, encrypted.Length - filled);
                    Require(count > 0); filled += count;
                }
                Require(file.ReadByte() == -1);
            }
            byte[] plaintext = null;
            try
            {
                plaintext = ProtectedData.Unprotect(encrypted, Entropy, DataProtectionScope.CurrentUser);
                byte[] authority = Encoding.ASCII.GetBytes(origin);
                Require(root.Length == 32 && authority.Length <= 32 && plaintext.Length == 2 + 32 + authority.Length + 43 + 1);
                Require(plaintext[0] == 1 && plaintext[1] == authority.Length);
                for (int index = 0; index < root.Length; index++) { Require(plaintext[2 + index] == root[index]); }
                for (int index = 0; index < authority.Length; index++) { Require(plaintext[34 + index] == authority[index]); }
                int offset = 34 + authority.Length; char[] token = new char[43];
                for (int index = 0; index < token.Length; index++) { token[index] = (char)plaintext[offset + index]; }
                if (!ValidToken(token)) { Array.Clear(token, 0, token.Length); throw new IOException("Credential encoding is invalid."); }
                byte state = plaintext[plaintext.Length - 1];
                if (state > 1) { Array.Clear(token, 0, token.Length); throw new IOException("Credential state is invalid."); }
                return new Credential(token, state == 1);
            }
            finally
            {
                Array.Clear(encrypted, 0, encrypted.Length);
                if (plaintext != null) { Array.Clear(plaintext, 0, plaintext.Length); }
            }
        }

        internal static bool ValidToken(char[] token)
        {
            if (token.Length != 43) { return false; }
            foreach (char value in token)
            {
                if (!(value >= 'A' && value <= 'Z') && !(value >= 'a' && value <= 'z') &&
                    !(value >= '0' && value <= '9') && value != '-' && value != '_') { return false; }
            }
            const string alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
            return (alphabet.IndexOf(token[token.Length - 1]) & 3) == 0;
        }

        internal void Write(string origin, byte[] root, Credential credential)
        {
            RequireSettled(); Require(root.Length == 32 && ValidToken(credential.Token));
            if (Present(current)) { using (FileStream checkedFile = OpenChecked(current)) { } }
            byte[] authority = Encoding.ASCII.GetBytes(origin); Require(authority.Length <= 32);
            byte[] plaintext = new byte[2 + 32 + authority.Length + 43 + 1];
            plaintext[0] = 1; plaintext[1] = (byte)authority.Length;
            Buffer.BlockCopy(root, 0, plaintext, 2, 32); Buffer.BlockCopy(authority, 0, plaintext, 34, authority.Length);
            for (int index = 0; index < 43; index++) { plaintext[34 + authority.Length + index] = (byte)credential.Token[index]; }
            plaintext[plaintext.Length - 1] = credential.Retiring ? (byte)1 : (byte)0;
            byte[] encrypted = null;
            try
            {
                encrypted = ProtectedData.Protect(plaintext, Entropy, DataProtectionScope.CurrentUser);
                Require(encrypted.Length > 0 && encrypted.Length <= Limit);
                using (FileStream file = new FileStream(pending, FileMode.CreateNew, FileSystemRights.FullControl,
                    FileShare.None, Limit, FileOptions.WriteThrough, FileSecurity()))
                {
                    VerifyPrivate(file.GetAccessControl()); file.Write(encrypted, 0, encrypted.Length); file.Flush(true);
                }
                VerifyPrivateDirectory();
                if (Present(current)) { File.Replace(pending, current, null, false); }
                else { File.Move(pending, current); }
                using (FileStream checkedFile = OpenChecked(current)) { }
            }
            finally
            {
                Array.Clear(plaintext, 0, plaintext.Length);
                if (encrypted != null) { Array.Clear(encrypted, 0, encrypted.Length); }
                // A failed replacement can have changed either name. Preserve
                // the fixed pending file for explicit recovery; never erase
                // the last encrypted credential or evidence of interruption.
            }
        }

        internal void Forget()
        {
            VerifyPrivateDirectory();
            foreach (string path in new[] { current, pending })
            {
                if (Present(path)) { using (FileStream checkedFile = OpenChecked(path, true)) { } }
            }
            if (Present(current)) { File.Delete(current); }
            if (Present(pending)) { File.Delete(pending); }
        }
    }
}
