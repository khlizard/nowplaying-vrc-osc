using System.Net;
using System.Net.Sockets;
using System.Text;

namespace NowPlayingVrcOsc.Core;

internal sealed class OscSender : IDisposable
{
    private readonly UdpClient _udp = new();
    private readonly IPEndPoint _endpoint;

    public OscSender(string host, int port)
    {
        _endpoint = new IPEndPoint(IPAddress.Parse(host), port);
    }

    public void Send(string address, params object[] args)
    {
        var packet = BuildPacket(address, args);
        _udp.Send(packet, packet.Length, _endpoint);
    }

    private static byte[] BuildPacket(string address, object[] args)
    {
        var addrBytes = PadString(address, Encoding.ASCII);
        var typeTagBuilder = new System.Text.StringBuilder(",");
        var argSegments = new List<byte[]>();

        foreach (var arg in args)
        {
            switch (arg)
            {
                case string s:
                    typeTagBuilder.Append('s');
                    argSegments.Add(PadString(s, Encoding.UTF8));
                    break;
                case bool b:
                    typeTagBuilder.Append(b ? 'T' : 'F');
                    // OSC T/F type tags carry no value bytes
                    break;
                case int i:
                    typeTagBuilder.Append('i');
                    argSegments.Add([(byte)(i >> 24), (byte)(i >> 16), (byte)(i >> 8), (byte)i]);
                    break;
            }
        }

        var typeBytes = PadString(typeTagBuilder.ToString(), Encoding.ASCII);
        var totalLen = addrBytes.Length + typeBytes.Length + argSegments.Sum(b => b.Length);
        var result = new byte[totalLen];
        var offset = 0;

        addrBytes.CopyTo(result, offset); offset += addrBytes.Length;
        typeBytes.CopyTo(result, offset); offset += typeBytes.Length;
        foreach (var seg in argSegments) { seg.CopyTo(result, offset); offset += seg.Length; }

        return result;
    }

    // Null-terminate and pad to 4-byte boundary (OSC string encoding)
    private static byte[] PadString(string s, Encoding encoding)
    {
        var raw = encoding.GetBytes(s);
        var nullTermLen = raw.Length + 1;
        var paddedLen = ((nullTermLen + 3) / 4) * 4;
        var result = new byte[paddedLen]; // zero-initialized = null padding
        raw.CopyTo(result, 0);
        return result;
    }

    public void Dispose() => _udp.Dispose();
}
