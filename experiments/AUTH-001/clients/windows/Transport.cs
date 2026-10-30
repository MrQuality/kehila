using System;
using System.Globalization;
using System.IO;
using System.Net;
using System.Net.Http;
using System.Net.Http.Headers;
using System.Net.Security;
using System.Runtime.Serialization.Json;
using System.Security.Authentication;
using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;
using System.Threading;
using System.Threading.Tasks;
using System.Xml;

namespace Kehila.Auth001.Client
{
    internal sealed class Reply : IDisposable
    {
        internal readonly HttpStatusCode Status;
        internal readonly byte[] Body;
        internal readonly int Length;
        internal Reply(HttpStatusCode status, byte[] body, int length)
        { Status = status; Body = body; Length = length; }
        public void Dispose() { Array.Clear(Body, 0, Body.Length); }
    }

    // Windows client only. Platform certificate validation must succeed first;
    // the selected issuer pin is an additional restriction, never a trust bypass.
    internal sealed class Transport : IDisposable
    {
        private readonly HttpClient client;
        private readonly byte[] root;
        private readonly SemaphoreSlim requestSlot = new SemaphoreSlim(1, 1);
        internal readonly string Origin;

        internal Transport(string origin, byte[] root)
        {
            const string prefix = "https://127.0.0.1:";
            int port;
            if (!origin.StartsWith(prefix, StringComparison.Ordinal) ||
                !Int32.TryParse(origin.Substring(prefix.Length), NumberStyles.None, CultureInfo.InvariantCulture, out port) ||
                port < 1024 || port > 65535 || origin != prefix + port.ToString(CultureInfo.InvariantCulture) || root.Length != 32)
            { throw new ArgumentException("An exact loopback origin and issuer fingerprint are required."); }
            Origin = origin; this.root = (byte[])root.Clone();
            HttpClientHandler handler = new HttpClientHandler();
            handler.AllowAutoRedirect = false; handler.UseProxy = false;
            handler.UseCookies = false; handler.UseDefaultCredentials = false;
            handler.Credentials = null; handler.AutomaticDecompression = DecompressionMethods.None;
            handler.MaxResponseHeadersLength = 8; handler.SslProtocols = SslProtocols.Tls13;
            handler.ServerCertificateCustomValidationCallback = Validate;
            client = new HttpClient(handler, true); client.Timeout = Timeout.InfiniteTimeSpan;
        }

        private bool Validate(HttpRequestMessage request, X509Certificate2 certificate, X509Chain chain, SslPolicyErrors errors)
        {
            if (errors != SslPolicyErrors.None || certificate == null || chain == null || chain.ChainElements.Count == 0 ||
                request.RequestUri.Scheme != "https" || request.RequestUri.GetLeftPart(UriPartial.Authority) != Origin)
            { return false; }
            using (SHA256 digest = SHA256.Create())
            {
                byte[] selected = digest.ComputeHash(chain.ChainElements[chain.ChainElements.Count - 1].Certificate.RawData);
                int difference = 0;
                for (int index = 0; index < root.Length; index++) { difference |= root[index] ^ selected[index]; }
                return difference == 0;
            }
        }

        private static void Password(XmlDictionaryWriter writer, string field, char[] value)
        {
            if (value == null || value.Length > 1024) { throw new ArgumentException("Password exceeds client input bound."); }
            writer.WriteStartElement(field); writer.WriteAttributeString("type", "string");
            writer.WriteChars(value, 0, value.Length); writer.WriteEndElement();
        }

        internal async Task<Reply> Send(string operation, Credential credential, char[] password, char[] replacement)
        {
            bool read = operation == "state";
            if (operation != "login" && operation != "logout" && operation != "password" && operation != "revoke-others" && !read)
            { throw new ArgumentException("Unknown client operation."); }
            if ((credential == null) != (operation == "login")) { throw new ArgumentException("Credential kind mismatch."); }
            if (credential != null && (credential.Retiring && operation != "logout" || !CredentialStore.ValidToken(credential.Token)))
            { throw new IOException("Credential is not eligible for this operation."); }
            if (!requestSlot.Wait(0)) { throw new IOException("A client request is already in progress."); }
            byte[] input = new byte[4096]; byte[] output = new byte[1025]; bool returned = false;
            try
            {
                using (CancellationTokenSource deadline = new CancellationTokenSource(TimeSpan.FromSeconds(5)))
                using (HttpRequestMessage request = new HttpRequestMessage(read ? HttpMethod.Get : HttpMethod.Post,
                    Origin + "/auth/cli/" + operation))
                {
                    request.Version = new Version(1, 1); request.Headers.ConnectionClose = true;
                    if (credential != null)
                    {
                        // HttpClient requires an immutable header string. Owned
                        // buffers are wiped; library/string copies are not claimed erased.
                        request.Headers.Authorization = new AuthenticationHeaderValue("Bearer", new string(credential.Token));
                    }
                    if (!read)
                    {
                        int length;
                        using (MemoryStream stream = new MemoryStream(input, 0, input.Length, true, true))
                        {
                            stream.SetLength(0);
                            using (XmlDictionaryWriter writer = JsonReaderWriterFactory.CreateJsonWriter(stream))
                            {
                                writer.WriteStartElement("root"); writer.WriteAttributeString("type", "object");
                                if (operation == "login" || operation == "revoke-others") { Password(writer, "password", password); }
                                if (operation == "password")
                                { Password(writer, "current", password); Password(writer, "replacement", replacement); }
                                writer.WriteEndElement(); writer.Flush(); length = checked((int)stream.Length);
                            }
                        }
                        request.Content = new ByteArrayContent(input, 0, length);
                        request.Content.Headers.ContentType = new MediaTypeHeaderValue("application/json");
                    }
                    using (HttpResponseMessage response = await client.SendAsync(request, HttpCompletionOption.ResponseHeadersRead, deadline.Token).ConfigureAwait(false))
                    using (CancellationTokenRegistration abort = deadline.Token.Register(response.Dispose))
                    {
                        if (response.Content.Headers.ContentLength > 1024 || response.Content.Headers.ContentType == null ||
                            response.Content.Headers.ContentType.MediaType != "application/json")
                        { throw new IOException("Unexpected response envelope."); }
                        int filled = 0;
                        using (Stream stream = await response.Content.ReadAsStreamAsync().ConfigureAwait(false))
                        {
                            while (filled < output.Length)
                            {
                                int count = await stream.ReadAsync(output, filled, output.Length - filled, deadline.Token).ConfigureAwait(false);
                                if (count == 0) { break; }
                                filled += count;
                            }
                        }
                        deadline.Token.ThrowIfCancellationRequested();
                        if (filled > 1024) { throw new IOException("Response exceeds client input bound."); }
                        returned = true; return new Reply(response.StatusCode, output, filled);
                    }
                }
            }
            finally
            {
                Array.Clear(input, 0, input.Length);
                if (!returned) { Array.Clear(output, 0, output.Length); }
                requestSlot.Release();
            }
        }

        public void Dispose() { client.Dispose(); requestSlot.Dispose(); }
    }
}
