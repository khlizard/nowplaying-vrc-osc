namespace NowPlayingVrcOsc.Core;

public sealed class VrcChatboxSender : IDisposable
{
    private readonly OscSender _osc;
    private readonly bool _notification;
    private readonly bool _indicator;

    public VrcChatboxSender(string host, int port, bool notification = false, bool indicator = false)
    {
        _osc = new OscSender(host, port);
        _notification = notification;
        _indicator = indicator;
    }

    public void Send(string message)
    {
        if (_indicator)
            _osc.Send("/chatbox/typing", true);

        _osc.Send("/chatbox/input", message, true, _notification);

        if (_indicator)
            _osc.Send("/chatbox/typing", false);
    }

    public void Dispose() => _osc.Dispose();
}
