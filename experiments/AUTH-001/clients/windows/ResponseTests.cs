using System;
using System.IO;
using System.Net;
using System.Text;
using System.Xml;

namespace Kehila.Auth001.Client
{
    internal static class ResponseTests
    {
        private static int caseNumber;
        private static Reply Reply(string json, HttpStatusCode status = HttpStatusCode.OK)
        {
            byte[] bytes = Encoding.UTF8.GetBytes(json);
            return new Reply(status, bytes, bytes.Length);
        }
        private static void Reject(string json, HttpStatusCode status = HttpStatusCode.OK)
        {
            caseNumber++;
            bool rejected = false;
            try { using (Reply reply = Reply(json, status)) { Program.Fields(reply); } }
            catch (IOException) { rejected = true; }
            catch (XmlException) { rejected = true; }
            if (!rejected) throw new Exception("Unexpected response accepted in case " + caseNumber);
        }
        private static int Main()
        {
            using (Reply reply = Reply("{\"active\":true}"))
            { if (Program.Fields(reply)["active"] != "true") throw new Exception("Boolean response failed."); }
            using (Reply reply = Reply("{}"))
            { if (Program.Fields(reply).Count != 0) throw new Exception("Empty response failed."); }
            const string failure = "{\"error\":\"request rejected\"}";
            foreach (int status in new int[] { 400, 401, 503, 599 })
            {
                using (Reply reply = Reply(failure, (HttpStatusCode)status))
                { if (Program.Fields(reply).Count != 0) throw new Exception("Fixed rejection response failed."); }
            }
            Reject(failure);
            Reject(failure, (HttpStatusCode)399);
            Reject(failure, (HttpStatusCode)600);
            Reject("{\"error\":\"different message\"}", HttpStatusCode.Unauthorized);
            Reject("{\"error\":true}", HttpStatusCode.Unauthorized);
            Reject("{\"error\":\"request rejected\",\"active\":true}", HttpStatusCode.Unauthorized);
            Reject(failure + " {}", HttpStatusCode.Unauthorized);
            Reject("{\"__type\":\"ignored\",\"active\":true}");
            Reject("{\"active\":true,\"active\":false}");
            Reject("{\"active\":\"true\"}");
            Reject("{\"active\":{\"value\":true}}");
            Reject("{\"unknown\":true}");
            Reject("[]");
            Reject("{\"active\":true} {}");
            Console.WriteLine("PASS: closed bounded JSON response shapes and metadata rejection.");
            return 0;
        }
    }
}
