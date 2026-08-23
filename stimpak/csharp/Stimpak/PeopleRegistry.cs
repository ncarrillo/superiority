using System.Collections.ObjectModel;

namespace Stimpak;

/// <summary>
/// who is in this connection, and who is in which channel.
/// </summary>
/// <remarks>
/// <para>
/// feed every event through <see cref="Apply"/> and bind to what comes out.
/// each channel membership is one instance for the life of the connection, so
/// a presence that arrives after the join that announced it updates in place
/// rather than forcing the roster to be rebuilt. A stable presence id can map
/// the same account's memberships across channels.
/// </para>
/// <para>
/// that rebuild is the reason this exists: the server re-sends a whole roster
/// whenever any single member changes, so a consumer that clears and refills a
/// collection per snapshot discards hundreds of rows — and any selection or
/// scroll position with them — to reflect one person going idle.
/// </para>
/// <para>
/// nothing here is thread-safe, deliberately. call <see cref="Apply"/> from the
/// same thread that reads it — for a ui, the one that owns the dispatcher — so
/// change notifications arrive where the bindings live.
/// </para>
/// </remarks>
public sealed class PeopleRegistry
{
    private readonly Dictionary<MemberKey, Person> _people = [];
    private readonly Dictionary<uint, HashSet<Person>> _presence = [];
    private readonly Dictionary<byte, ObservableCollection<Person>> _channels = [];

    /// <summary>everyone this session has heard of.</summary>
    public IReadOnlyCollection<Person> People => _people.Values;

    /// <summary>
    /// the membership of a channel, reconciled in place. safe to bind to
    /// before the channel is joined; it fills in when the roster lands.
    /// </summary>
    public ObservableCollection<Person> Channel(byte channelIndex)
    {
        if (!_channels.TryGetValue(channelIndex, out var roster))
        {
            roster = [];
            _channels[channelIndex] = roster;
        }
        return roster;
    }

    /// <summary>
    /// the membership identified by a channel and handle, created if this is
    /// the first mention. Handles are not unique outside their channel.
    /// </summary>
    public Person Get(byte channelIndex, uint handle)
    {
        var key = new MemberKey(channelIndex, handle);
        if (!_people.TryGetValue(key, out var person))
        {
            person = new Person(channelIndex, handle);
            _people[key] = person;
        }
        return person;
    }

    /// <summary>
    /// every channel membership known to belong to one stable Battle.net
    /// presence. One account can appear in more than one channel.
    /// </summary>
    public IReadOnlyCollection<Person> FindByPresenceId(uint presenceId) =>
        _presence.TryGetValue(presenceId, out var people) ? people.ToArray() : [];

    public void Apply(SC2Event next)
    {
        switch (next)
        {
            case Joined joined:
                ForgetChannel(joined.ChannelIndex);
                break;

            case RosterReceived roster:
                Reconcile(roster.ChannelIndex, roster.Users);
                break;

            case MemberJoined member:
                Absorb(member.ChannelIndex, member.User);
                Add(member.ChannelIndex, member.User.Handle);
                break;

            case MemberLeft member:
                Absorb(member.ChannelIndex, member.User);
                Channel(member.ChannelIndex).Remove(Get(member.ChannelIndex, member.User.Handle));
                break;

            // a message describes its sender too, and sometimes knows more
            // than the roster did.
            case MessageReceived message:
                Absorb(message.ChannelIndex, message.Sender);
                break;

            case Left left:
                ForgetChannel(left.ChannelIndex);
                break;

            case StageChanged { Stage: Stage.Disconnected or Stage.WebAuthentication }:
            case SessionEnded:
                Reset();
                break;
        }
    }

    private void Absorb(byte channelIndex, User user)
    {
        var person = Get(channelIndex, user.Handle);
        var previous = person.PresenceId;
        person.Absorb(user);
        if (previous == person.PresenceId)
        {
            return;
        }
        if (previous is { } old && _presence.TryGetValue(old, out var oldPeople))
        {
            oldPeople.Remove(person);
            if (oldPeople.Count == 0)
            {
                _presence.Remove(old);
            }
        }
        if (person.PresenceId is { } current)
        {
            if (!_presence.TryGetValue(current, out var currentPeople))
            {
                currentPeople = [];
                _presence[current] = currentPeople;
            }
            currentPeople.Add(person);
        }
    }

    private void Add(byte channelIndex, uint handle)
    {
        var roster = Channel(channelIndex);
        var person = Get(channelIndex, handle);
        if (!roster.Contains(person))
        {
            roster.Add(person);
        }
    }

    /// <summary>
    /// brings a channel's membership in line with a snapshot by adding and
    /// removing only what differs, so every unchanged row keeps its instance.
    /// </summary>
    private void Reconcile(byte channelIndex, IReadOnlyList<User> users)
    {
        foreach (var user in users)
        {
            Absorb(channelIndex, user);
        }

        var roster = Channel(channelIndex);
        var present = users.Select(user => user.Handle).ToHashSet();
        for (var index = roster.Count - 1; index >= 0; index--)
        {
            if (!present.Contains(roster[index].Handle))
            {
                roster.RemoveAt(index);
            }
        }

        var already = roster.Select(person => person.Handle).ToHashSet();
        foreach (var user in users.Where(user => !already.Contains(user.Handle)))
        {
            roster.Add(Get(channelIndex, user.Handle));
        }
    }

    private void Reset()
    {
        _people.Clear();
        _presence.Clear();
        foreach (var roster in _channels.Values)
        {
            roster.Clear();
        }
    }

    private void ForgetChannel(byte channelIndex)
    {
        Channel(channelIndex).Clear();
        var forgotten = _people
            .Where(entry => entry.Key.ChannelIndex == channelIndex)
            .Select(entry => entry.Value)
            .ToArray();
        foreach (var person in forgotten)
        {
            _people.Remove(new MemberKey(channelIndex, person.Handle));
            if (person.PresenceId is not { } presenceId ||
                !_presence.TryGetValue(presenceId, out var people))
            {
                continue;
            }
            people.Remove(person);
            if (people.Count == 0)
            {
                _presence.Remove(presenceId);
            }
        }
    }

    private readonly record struct MemberKey(byte ChannelIndex, uint Handle);
}
