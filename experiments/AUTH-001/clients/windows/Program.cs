using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Net;
using System.Runtime.Serialization.Json;
using System.Xml;

namespace Kehila.Auth001.Client
{
    internal static class Program
    {
        private static void Require(bool value) { if (!value) { throw new IOException("Client response was not confirmed."); } }

        private static byte[] Fingerprint(string text)
        {
            Require(text.Length == 64); byte[] root = new byte[32];
            for (int index = 0; index < root.Length; index++)
            {
                string pair = text.Substring(index * 2, 2);
                Require(pair == pair.ToUpperInvariant());
                root[index] = Byte.Parse(pair, NumberStyles.AllowHexSpecifier, CultureInfo.InvariantCulture);
            }
            return root;
        }

        private static char[] ReadPassword(string prompt, bool redirected)
        {
            if (!redirected && Console.IsInputRedirected) { throw new IOException("Redirected passwords require --stdin."); }
            if (!redirected) { Console.Write(prompt); }
            char[] buffer = new char[1024]; int length = 0;
            try
            {
                while (true)
                {
                    char value;
                    if (redirected)
                    {
                        int read = Console.In.Read();
                        Require(read >= 0); value = (char)read;
                        if (value == '\r') { Require(Console.In.Read() == '\n'); break; }
                        if (value == '\n') { break; }
                    }
                    else
                    {
                        ConsoleKeyInfo key = Console.ReadKey(true);
                        if (key.Key == ConsoleKey.Enter) { Console.WriteLine(); break; }
                        if (key.Key == ConsoleKey.Backspace)
                        { if (length > 0) { buffer[--length] = '\0'; } continue; }
                        if (key.KeyChar == '\0') { continue; }
                        value = key.KeyChar;
                    }
                    Require(length < buffer.Length); buffer[length++] = value;
                }
                char[] result = new char[length]; Array.Copy(buffer, result, length); return result;
            }
            finally { Array.Clear(buffer, 0, buffer.Length); }
        }

        internal static Dictionary<string, string> Fields(Reply reply)
        {
            XmlDictionaryReaderQuotas quotas = new XmlDictionaryReaderQuotas();
            quotas.MaxDepth = 4; quotas.MaxStringContentLength = 1024; quotas.MaxArrayLength = 8;
            quotas.MaxBytesPerRead = 1024; quotas.MaxNameTableCharCount = 256;
            Dictionary<string, string> fields = new Dictionary<string, string>(StringComparer.Ordinal);
            List<string> order = new List<string>();
            using (XmlDictionaryReader reader = JsonReaderWriterFactory.CreateJsonReader(reply.Body, 0, reply.Length, quotas))
            {
                Require(reader.MoveToContent() == XmlNodeType.Element && reader.LocalName == "root" &&
                    reader.NamespaceURI.Length == 0 && reader.Prefix.Length == 0 && reader.AttributeCount == 1 && reader.GetAttribute("type") == "object");
                bool empty = reader.IsEmptyElement; reader.ReadStartElement();
                if (!empty)
                {
                    while (reader.MoveToContent() == XmlNodeType.Element)
                    {
                        string name = reader.LocalName; string type = reader.GetAttribute("type");
                        Require(reader.NamespaceURI.Length == 0 && reader.Prefix.Length == 0 && reader.AttributeCount == 1);
                        Require(fields.Count < 3 && !fields.ContainsKey(name));
                        string expected = name == "bearer" || name == "error" ? "string" : name == "count" ? "number" : "boolean";
                        Require(type == expected && (name == "bearer" || name == "error" || name == "count" || name == "active" ||
                            name == "signed_out" || name == "audit_confirmed" || name == "password_changed" || name == "other_sessions_revoked"));
                        fields.Add(name, reader.ReadElementContentAsString());
                        order.Add(name);
                    }
                    reader.ReadEndElement();
                }
                Require(reader.MoveToContent() == XmlNodeType.None);
            }
            // The framework JSON reader can report EOF at the first object
            // while ignoring trailing bytes. This client accepts the server's
            // fixed canonical encoding only; a bounded BCL re-encode verifies
            // the entire original envelope, including metadata/trailing data.
            byte[] canonical = new byte[1024];
            try
            {
                using (MemoryStream stream = new MemoryStream(canonical, 0, canonical.Length, true, true))
                {
                    stream.SetLength(0);
                    using (XmlDictionaryWriter writer = JsonReaderWriterFactory.CreateJsonWriter(stream))
                    {
                        writer.WriteStartElement("root"); writer.WriteAttributeString("type", "object");
                        foreach (string name in order)
                        {
                            writer.WriteStartElement(name);
                            writer.WriteAttributeString("type", name == "bearer" || name == "error" ? "string" : name == "count" ? "number" : "boolean");
                            writer.WriteString(fields[name]); writer.WriteEndElement();
                        }
                        writer.WriteEndElement(); writer.Flush();
                        Require(stream.Length == reply.Length);
                        for (int index = 0; index < reply.Length; index++) Require(canonical[index] == reply.Body[index]);
                    }
                }
            }
            finally { Array.Clear(canonical, 0, canonical.Length); }
            if (fields.ContainsKey("error"))
            {
                // Normalize only the server's fixed canonical rejection envelope.
                // Error status and exact shape are required; never print server text.
                int status = (int)reply.Status;
                Require(status >= 400 && status <= 599 && fields.Count == 1 && fields["error"] == "request rejected");
                fields.Clear();
            }
            return fields;
        }

        private static bool Is(Dictionary<string, string> fields, string name, string value)
        { string actual; return fields.TryGetValue(name, out actual) && actual == value; }

        private static int Execute(string[] args)
        {
            if (args.Length == 1 && args[0] == "forget")
            {
                CredentialStore.ForCurrentUser().Forget();
                Console.WriteLine("Local credential forgotten. This does not confirm server logout or its audit."); return 0;
            }
            if (args.Length < 3 || args.Length > 4 || args.Length == 4 && args[3] != "--stdin")
            {
                Console.WriteLine("Usage: owner-client <login|state|logout|password|revoke-others> <https://127.0.0.1:port> <uppercase issuer SHA-256> [--stdin]");
                Console.WriteLine("Use owner-client forget to discard only the local credential. Passwords never belong in arguments."); return 2;
            }
            string operation = args[0]; bool redirected = args.Length == 4;
            Require(operation == "login" || operation == "state" || operation == "logout" || operation == "password" || operation == "revoke-others");
            byte[] root = Fingerprint(args[2]);
            CredentialStore store = CredentialStore.ForCurrentUser();
            char[] password = null; char[] replacement = null;
            bool passwordSent = false; bool passwordOutcomeKnown = false;
            try
            {
                using (Transport transport = new Transport(args[1], root))
                using (Credential credential = store.Read(transport.Origin, root))
                {
                    Require((credential == null) == (operation == "login"));
                    if (credential != null && credential.Retiring && operation != "logout")
                    { Console.WriteLine("Signed out locally; retry logout audit or use forget. No session probe was sent."); return 1; }
                    if (operation == "login" || operation == "password" || operation == "revoke-others")
                    { password = ReadPassword(operation == "login" ? "Owner password: " : "Current password: ", redirected); }
                    if (operation == "password") { replacement = ReadPassword("New password: ", redirected); }
                    if (operation == "logout")
                    {
                        // Refuse any later business/session probe even if the
                        // response is lost before local revocation is confirmed.
                        using (Credential retiring = new Credential((char[])credential.Token.Clone(), true))
                        { store.Write(transport.Origin, root, retiring); }
                    }
                    passwordSent = operation == "password";
                    Reply reply = transport.Send(operation, credential, password, replacement).GetAwaiter().GetResult();
                    using (reply)
                    {
                        Dictionary<string, string> fields = Fields(reply);
                        if (reply.Status == HttpStatusCode.OK)
                        {
                            if (operation == "login")
                            {
                                Require(fields.Count == 1 && fields.ContainsKey("bearer"));
                                using (Credential issued = new Credential(fields["bearer"].ToCharArray(), false))
                                { Require(CredentialStore.ValidToken(issued.Token)); store.Write(transport.Origin, root, issued); }
                                Console.WriteLine("Signed in; credential protected for this Windows user.");
                            }
                            else if (operation == "state")
                            {
                                Require(fields.Count == 1 && (Is(fields, "active", "true") || Is(fields, "active", "false")));
                                Console.WriteLine(Is(fields, "active", "true") ? "Session active." : "Session signed out; only logout audit retry remains.");
                            }
                            else if (operation == "logout")
                            {
                                Require(fields.Count == 2 && Is(fields, "signed_out", "true") && Is(fields, "audit_confirmed", "true"));
                                store.Forget(); Console.WriteLine("Signed out; audit confirmed.");
                            }
                            else if (operation == "password")
                            {
                                Require(fields.Count == 1 && Is(fields, "password_changed", "true"));
                                passwordOutcomeKnown = true;
                                store.Forget(); Console.WriteLine("Password changed; sign in again using the new password.");
                            }
                            else
                            {
                                int count;
                                Require(fields.Count == 3 && Is(fields, "other_sessions_revoked", "true") && Is(fields, "audit_confirmed", "true") &&
                                    fields.ContainsKey("count") && Int32.TryParse(fields["count"], out count) && count >= 0 && count <= 63);
                                Console.WriteLine("Other sessions signed out; audit confirmed. This session remains active.");
                            }
                            return 0;
                        }
                        if (reply.Status == HttpStatusCode.ServiceUnavailable && operation == "logout" &&
                            fields.Count == 2 && Is(fields, "signed_out", "true") && Is(fields, "audit_confirmed", "false"))
                        { Console.WriteLine("Signed out; audit unconfirmed. Retry logout or use forget."); return 1; }
                        if (reply.Status == HttpStatusCode.ServiceUnavailable && operation == "revoke-others" &&
                            fields.Count == 2 && Is(fields, "other_sessions_revoked", "true") && Is(fields, "audit_confirmed", "false"))
                        { Console.WriteLine("Selected sessions signed out; audit unconfirmed. Retry this operation for its original set."); return 1; }
                        Require(fields.Count == 0);
                        if ((int)reply.Status >= 400 && (int)reply.Status < 500) passwordOutcomeKnown = true;
                        if (operation == "password" && !passwordOutcomeKnown)
                        { store.Forget(); Console.WriteLine("Change outcome uncertain. Reconcile with a fresh login using the chosen new password; do not automatically repeat it."); }
                        else if (operation == "state" && reply.Status == HttpStatusCode.Unauthorized)
                        { store.Forget(); Console.WriteLine("Session unavailable; local credential discarded. No audit outcome is implied."); }
                        else Console.WriteLine(reply.Status == HttpStatusCode.Unauthorized ? "Credential or password was not accepted." : "Request unavailable or rejected. No success is claimed.");
                        return 1;
                    }
                }
            }
            catch
            {
                if (passwordSent && !passwordOutcomeKnown)
                {
                    Console.WriteLine("Password-change outcome uncertain. Forgetting the old local credential; try a fresh login with the chosen new password. Do not automatically repeat the change.");
                    store.Forget();
                }
                throw;
            }
            finally
            {
                if (password != null) Array.Clear(password, 0, password.Length);
                if (replacement != null) Array.Clear(replacement, 0, replacement.Length);
            }
        }

        private static int Main(string[] args)
        {
            try { return Execute(args); }
            catch { Console.WriteLine("The result was not confirmed. No automatic retry was performed. Check session recovery or explicitly forget the local credential."); return 1; }
        }
    }
}
