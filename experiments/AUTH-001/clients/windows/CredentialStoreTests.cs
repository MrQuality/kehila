using System;
using System.IO;
using System.Security.AccessControl;
using System.Security.Principal;

namespace Kehila.Auth001.Client
{
    // Run only in a fresh external directory. No certificate trust or network.
    internal static class CredentialStoreTests
    {
        private static void Check(bool value) { if (!value) { throw new Exception("Store assertion failed."); } }
        private static void Reject(Action action)
        {
            bool rejected = false;
            try { action(); }
            catch (IOException) { rejected = true; }
            catch (System.Security.Cryptography.CryptographicException) { rejected = true; }
            Check(rejected);
        }

        private static int Main(string[] args)
        {
            if (args.Length != 1 || Directory.Exists(args[0]) || File.Exists(args[0])) { return 2; }
            // The caller owns the external parent and cleanup. A failed test is
            // retained for inspection, never recursively removed by this test.
            string directory = Path.GetFullPath(args[0]);
            CredentialStore store = new CredentialStore(directory);
            byte[] root = new byte[32]; root[0] = 1;
            const string origin = "https://127.0.0.1:16281";
            char[] token = new string('A', 43).ToCharArray();
            using (Credential source = new Credential(token, false))
            {
                Check(store.Read(origin, root) == null);
                store.Write(origin, root, source);
                using (Credential copy = store.Read(origin, root))
                { Check(!copy.Retiring && new string(copy.Token) == new string(source.Token)); }
                using (Credential retiring = new Credential(new string('A', 43).ToCharArray(), true))
                { store.Write(origin, root, retiring); }
                using (Credential copy = store.Read(origin, root)) { Check(copy.Retiring); }
                Reject(delegate { using (Credential copy = store.Read("https://127.0.0.1:16282", root)) { } });
                byte[] wrongRoot = new byte[32]; wrongRoot[0] = 2;
                Reject(delegate { using (Credential copy = store.Read(origin, wrongRoot)) { } });

                string current = Path.Combine(directory, "credential.dat");
                string pending = Path.Combine(directory, "credential.pending");
                File.Copy(current, pending);
                FileSecurity pendingSecurity = File.GetAccessControl(pending);
                pendingSecurity.SetAccessRuleProtection(true, true);
                File.SetAccessControl(pending, pendingSecurity);
                // A copied file uses the creator token's ambient owner. Measure
                // that separately; it is not a valid production-write fixture.
                bool copiedOwnerMatches;
                using (WindowsIdentity identity = WindowsIdentity.GetCurrent())
                { copiedOwnerMatches = identity.User.Equals(File.GetAccessControl(pending).GetOwner(typeof(SecurityIdentifier))); }
                Console.WriteLine("Copied pending fixture owner matches current user: " + copiedOwnerMatches);
                if (!copiedOwnerMatches) { Reject(delegate { store.Forget(); }); }
                File.Delete(pending); // Only the file this fresh fixture just created.
                FileSecurity ownedPending = new FileSecurity();
                using (WindowsIdentity identity = WindowsIdentity.GetCurrent())
                {
                    ownedPending.SetOwner(identity.User); ownedPending.SetAccessRuleProtection(true, false);
                    ownedPending.AddAccessRule(new FileSystemAccessRule(identity.User, FileSystemRights.FullControl, AccessControlType.Allow));
                }
                using (FileStream destination = new FileStream(pending, FileMode.CreateNew, FileSystemRights.FullControl,
                    FileShare.None, 4096, FileOptions.None, ownedPending))
                using (FileStream sourceFile = File.OpenRead(current)) { sourceFile.CopyTo(destination); }
                Reject(delegate { using (Credential copy = store.Read(origin, root)) { } });
                Reject(delegate { store.Write(origin, root, source); });
                Console.WriteLine("Checking interrupted-write recovery.");
                store.Forget();
                Check(!File.Exists(current) && !File.Exists(pending));

                store.Write(origin, root, source);
                byte[] encrypted = File.ReadAllBytes(current);
                encrypted[encrypted.Length / 2] ^= 1;
                File.WriteAllBytes(current, encrypted);
                Reject(delegate { using (Credential copy = store.Read(origin, root)) { } });
                Console.WriteLine("Checking corrupt-envelope removal.");
                store.Forget();

                store.Write(origin, root, source);
                FileSecurity weak = File.GetAccessControl(current);
                SecurityIdentifier users = new SecurityIdentifier(WellKnownSidType.BuiltinUsersSid, null);
                weak.AddAccessRule(new FileSystemAccessRule(
                    users, FileSystemRights.Read, AccessControlType.Allow));
                File.SetAccessControl(current, weak);
                Reject(delegate { using (Credential copy = store.Read(origin, root)) { } });
                Reject(delegate { store.Write(origin, root, source); });
                FileSecurity restored = File.GetAccessControl(current);
                restored.PurgeAccessRules(users);
                File.SetAccessControl(current, restored);
                Console.WriteLine("Checking restored-ACL removal.");
                store.Forget();

                Directory.CreateDirectory(current);
                Reject(delegate { using (Credential copy = store.Read(origin, root)) { } });
                Reject(delegate { store.Write(origin, root, source); });
                Directory.Delete(current);
            }
            Console.WriteLine("PASS: protected credential roundtrip, replacement, binding, corruption, interrupted write and ACL rejection.");
            return 0;
        }
    }
}
