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
        private static Reply Reply(string json)
        {
            byte[] bytes = Encoding.UTF8.GetBytes(json);
            return new Reply(HttpStatusCode.OK, bytes, bytes.Length);
        }
        private static void Reject(string json)
        {
            caseNumber++;
            bool rejected = false;
            try { using (Reply reply = Reply(json)) { Program.Fields(reply); } }
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
