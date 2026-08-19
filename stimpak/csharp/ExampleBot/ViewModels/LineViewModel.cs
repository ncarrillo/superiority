using Avalonia.Media;
using Stimpak;

namespace ExampleBot.ViewModels;

/// <summary>
/// one transcript row. the accent is resolved here rather than by a converter
/// in the view, because which colour a name takes is a rule about people, not
/// about layout.
/// </summary>
public sealed class LineViewModel(string time, string speaker, string body, IBrush accent)
    : ViewModelBase
{
    public string Time { get; } = time;
    public string Speaker { get; } = speaker;
    public string Body { get; } = body;
    public IBrush Accent { get; } = accent;
    public bool HasSpeaker => Speaker.Length > 0;

    public static LineViewModel Message(User sender, string body, IBrush accent) =>
        new(Stamp(), $"{sender.Name}:", body, accent);

    public static LineViewModel Whisper(string peer, string body, IBrush accent) =>
        new(Stamp(), $"whisper from {peer}:", body, accent);

    public static LineViewModel Notice(string body, IBrush accent) => new(Stamp(), "", body, accent);

    private static string Stamp() => DateTime.Now.ToString("h:mm tt");
}
