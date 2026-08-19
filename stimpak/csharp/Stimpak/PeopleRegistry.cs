using System.Collections.ObjectModel;

namespace Stimpak;

/// <summary>
/// who is in this session, and who is in which channel.
/// </summary>
/// <remarks>
/// <para>
/// feed every event through <see cref="Apply"/> and bind to what comes out.
/// each person is one instance for the life of the session, so a presence that
/// arrives after the join that announced it updates in place rather than
/// forcing the roster to be rebuilt.
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
    private readonly Dictionary<uint, Person> _people = [];
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

    /// <summary>the entry for a handle, created if this is the first mention.</summary>
    public Person Get(uint handle)
    {
        if (!_people.TryGetValue(handle, out var person))
        {
            person = new Person(handle);
            _people[handle] = person;
        }
        return person;
    }

    public void Apply(SC2Event next)
    {
        switch (next)
        {
            case RosterReceived roster:
                Reconcile(roster.ChannelIndex, roster.Users);
                break;

            case MemberJoined member:
                Absorb(member.User);
                Add(member.ChannelIndex, member.User.Handle);
                break;

            case MemberLeft member:
                Absorb(member.User);
                Channel(member.ChannelIndex).Remove(Get(member.User.Handle));
                break;

            // a message describes its sender too, and sometimes knows more
            // than the roster did.
            case MessageReceived message:
                Absorb(message.Sender);
                break;

            case Left left:
                Channel(left.ChannelIndex).Clear();
                break;

            case SessionEnded:
                _people.Clear();
                foreach (var roster in _channels.Values)
                {
                    roster.Clear();
                }
                break;
        }
    }

    private void Absorb(User user) => Get(user.Handle).Absorb(user);

    private void Add(byte channelIndex, uint handle)
    {
        var roster = Channel(channelIndex);
        var person = Get(handle);
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
            Absorb(user);
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
            roster.Add(Get(user.Handle));
        }
    }
}
