using System.Collections.ObjectModel;
using Avalonia.Media;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using ExampleBot.Services;
using Stimpak;

namespace ExampleBot.ViewModels;

public sealed partial class MainWindowViewModel : ViewModelBase, IDisposable
{
    private static readonly IBrush MutedBrush = Brush.Parse("#7d8fa8");
    private static readonly IBrush NoticeBrush = Brush.Parse("#6bc2f2");
    private static readonly IBrush OnlineBrush = Brush.Parse("#47d184");
    private static readonly IBrush ClanBrush = Brush.Parse("#f0aa64");
    private static readonly IBrush TextBrush = Brush.Parse("#d6e0f0");
    private static readonly IBrush DangerBrush = Brush.Parse("#ff6b63");
    private static readonly IBrush StructuralBrush = Brush.Parse("#b3133e5b");
    private static readonly IBrush FocusedBrush = Brush.Parse("#33a8f0");

    /// <summary>a bot can outrun a reader, so the window stays bounded.</summary>
    private const int TranscriptLimit = 500;

    private readonly IChatSession _session;
    private readonly CancellationTokenSource _stopping = new();
    private byte? _channel;

    public ObservableCollection<LineViewModel> Transcript { get; } = [];

    /// <summary>
    /// the roster of the joined channel, reconciled in place by the session's
    /// registry — so a presence arriving after the join that announced it
    /// updates a row instead of replacing every row.
    /// </summary>
    [ObservableProperty]
    private ObservableCollection<Person> _roster = [];

    [ObservableProperty]
    private string _stage = "CONNECTING";

    [ObservableProperty]
    private IBrush _stageBrush = MutedBrush;

    [ObservableProperty]
    private string _channelName = "";

    [ObservableProperty]
    private string _population = "";

    [ObservableProperty]
    private IBrush _composerBorder = StructuralBrush;

    [ObservableProperty]
    private string _warning = "";

    [ObservableProperty]
    [NotifyCanExecuteChangedFor(nameof(SendCommand))]
    private string _draft = "";

    [ObservableProperty]
    [NotifyCanExecuteChangedFor(nameof(SendCommand))]
    private bool _joined;

    /// <summary>the designer needs a parameterless constructor.</summary>
    public MainWindowViewModel()
        : this(new SampleSession())
    {
    }

    public MainWindowViewModel(IChatSession session)
    {
        _session = session;
        Warning = session.HasAuthWindow ? "" : "NO SIGN-IN WINDOW";
    }

    /// <summary>
    /// starts the session and consumes it. separate from construction so the
    /// caller decides when network work begins, and so the designer can build
    /// this without any.
    /// </summary>
    public void Start()
    {
        Note("Connecting to Battle.net…");
        _session.Connect();
        _ = Consume();
    }

    private async Task Consume()
    {
        try
        {
            await foreach (var next in _session.ReadEventsAsync(_stopping.Token))
            {
                Apply(next);
            }
        }
        catch (OperationCanceledException)
        {
        }
    }

    private bool CanSend => Joined && Draft.Trim().Length > 0;

    [RelayCommand(CanExecute = nameof(CanSend))]
    private void Send()
    {
        if (_channel is not { } channel)
        {
            return;
        }
        var body = Draft.Trim();
        Draft = "";
        try
        {
            _session.SendMessage(channel, body);
        }
        catch (StimpakException error)
        {
            Note(error.Message, DangerBrush);
        }
    }

    private void Apply(SC2Event next)
    {
        // the registry is not thread-safe by design; this runs on the ui
        // thread, which is where the bindings watching it live.
        _session.People.Apply(next);
        switch (next)
        {
            case StageChanged stage:
                Stage = Describe(stage.Stage);
                StageBrush = stage.Stage switch
                {
                    Stimpak.Stage.Connected => OnlineBrush,
                    Stimpak.Stage.Disconnected => DangerBrush,
                    _ => NoticeBrush,
                };
                if (stage.Stage == Stimpak.Stage.Connected)
                {
                    _session.JoinPublic(StimpakClient.DefaultPublicChannel);
                }
                break;

            case AuthenticationRequired auth:
                Note($"Sign in at {auth.Url}", NoticeBrush);
                break;

            case PublicChannelsReceived catalogue:
                var count = catalogue.Channels.Count;
                Note($"{count} public channel{(count == 1 ? "" : "s")} available");
                break;

            case Joined joined:
                _channel = joined.ChannelIndex;
                Joined = true;
                Roster = _session.People.Channel(joined.ChannelIndex);
                ChannelName = joined.Channel.Name;
                ComposerBorder = FocusedBrush;
                Note($"Joined {joined.Channel.Name}", NoticeBrush);
                break;

            case Left:
                _channel = null;
                Joined = false;
                ComposerBorder = StructuralBrush;
                Recount();
                Note("Left the channel");
                break;

            case RosterReceived:
                Recount();
                break;

            case MemberJoined member:
                Recount();
                Note($"{_session.People.Get(member.User.Handle).Name} joined", OnlineBrush);
                break;

            case MemberLeft member:
                Recount();
                Note($"{_session.People.Get(member.User.Handle).Name} left");
                break;

            case MessageReceived message:
                Append(LineViewModel.Message(message.Sender, message.Body, Accent(message.Sender)));
                if (message.Body == "!ping" && _channel is { } channel)
                {
                    _session.SendMessage(channel, $"pong, {message.Sender.Name}");
                }
                break;

            case WhisperReceived { Outgoing: false } whisper:
                Append(LineViewModel.Whisper(whisper.Peer, whisper.Body, ClanBrush));
                break;

            case CommandFailed failure:
                Note(failure.Message, DangerBrush);
                break;

            case SessionFailed failure:
                Note(failure.Message, DangerBrush);
                break;
        }
    }

    private void Recount() => Population = $"{Roster.Count(person => person.IsOnline)} ONLINE";

    private static IBrush Accent(User user) => user.ClanTag is null ? TextBrush : ClanBrush;

    private static string Describe(Stage stage) => stage switch
    {
        Stimpak.Stage.Disconnected => "DISCONNECTED",
        Stimpak.Stage.WebAuthentication => "SIGNING IN",
        Stimpak.Stage.GameUtilities => "HANDSHAKE",
        Stimpak.Stage.NativeAuthentication => "AUTHENTICATING",
        Stimpak.Stage.ChatBootstrap => "STARTING CHAT",
        Stimpak.Stage.Connected => "CONNECTED",
        _ => "UNKNOWN",
    };

    private void Note(string body, IBrush? accent = null) =>
        Append(LineViewModel.Notice(body, accent ?? MutedBrush));

    private void Append(LineViewModel line)
    {
        Transcript.Add(line);
        while (Transcript.Count > TranscriptLimit)
        {
            Transcript.RemoveAt(0);
        }
    }

    public void Dispose()
    {
        _stopping.Cancel();
        _session.Dispose();
        _stopping.Dispose();
    }
}
