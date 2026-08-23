using System.ComponentModel;
using System.Runtime.CompilerServices;

namespace Stimpak;

/// <summary>
/// one channel membership, as long as the connection lasts.
/// </summary>
/// <remarks>
/// presence is state, not something an event carries once. a join names
/// somebody before their profile exists, so the same person is described
/// repeatedly and each description may know more than the last. holding one
/// mutable instance per person — rather than constructing a new value per
/// event — is what lets a view bind once and see it fill in, instead of being
/// rebuilt every time anything changes.
/// </remarks>
public sealed class Person : INotifyPropertyChanged
{
    internal Person(byte channelIndex, uint handle)
    {
        ChannelIndex = channelIndex;
        Handle = handle;
        _name = $"User {handle}";
    }

    /// <summary>the joined channel that owns <see cref="Handle"/>.</summary>
    public byte ChannelIndex { get; }

    /// <summary>identifies this membership within its channel.</summary>
    public uint Handle { get; }

    private uint? _presenceId;

    /// <summary>identifies the account across channels, once the server says.</summary>
    public uint? PresenceId
    {
        get => _presenceId;
        private set => Set(ref _presenceId, value);
    }

    private string _name;

    /// <summary>as the client shows it, clan tag and all.</summary>
    public string Name
    {
        get => _name;
        private set => Set(ref _name, value);
    }

    private string? _clanTag;

    public string? ClanTag
    {
        get => _clanTag;
        private set => Set(ref _clanTag, value);
    }

    private Presence _presence = Presence.Unknown;

    public Presence Presence
    {
        get => _presence;
        private set => Set(ref _presence, value);
    }

    public bool IsOnline => Presence is not (Presence.Offline or Presence.Unknown);

    /// <summary>
    /// takes whatever this description knows. a later description that knows
    /// less — a bare join after a full roster — must not erase what is already
    /// established, so absent fields are ignored rather than written as null.
    /// </summary>
    internal void Absorb(User user)
    {
        if (user.PresenceId is not null)
        {
            PresenceId = user.PresenceId;
        }
        if (!string.IsNullOrEmpty(user.Name))
        {
            Name = user.Name;
        }
        if (user.ClanTag is not null)
        {
            ClanTag = user.ClanTag;
        }
        if (user.Presence != Presence.Unknown)
        {
            Presence = user.Presence;
        }
    }

    public event PropertyChangedEventHandler? PropertyChanged;

    private void Set<T>(ref T field, T value, [CallerMemberName] string? name = null)
    {
        if (EqualityComparer<T>.Default.Equals(field, value))
        {
            return;
        }
        field = value;
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
        if (name == nameof(Presence))
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(IsOnline)));
        }
    }
}
