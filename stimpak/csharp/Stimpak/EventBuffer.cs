using System.Runtime.CompilerServices;

namespace Stimpak;

/// <summary>
/// A bounded single-publisher stream. Roster snapshots are replaceable state,
/// so an unread snapshot for the same channel is updated in place. Every other
/// event retains arrival order until the configured bound is reached.
/// </summary>
internal sealed class EventBuffer(int capacity)
{
    private readonly int _capacity = capacity > 0
        ? capacity
        : throw new ArgumentOutOfRangeException(nameof(capacity));
    private readonly object _gate = new();
    private readonly LinkedList<SC2Event> _queue = [];
    private readonly Dictionary<byte, LinkedListNode<SC2Event>> _rosters = [];
    private readonly SemaphoreSlim _ready = new(0);
    private bool _completed;
    private long _dropped;
    private int _waiters;

    internal long DroppedCount => Interlocked.Read(ref _dropped);

    /// <summary>returns the number dropped by this publication: zero or one.</summary>
    internal int Publish(SC2Event next)
    {
        var wake = false;
        var dropped = 0;
        lock (_gate)
        {
            if (_completed)
            {
                return 0;
            }
            if (next is RosterReceived roster &&
                _rosters.TryGetValue(roster.ChannelIndex, out var existing))
            {
                existing.Value = next;
                return 0;
            }

            wake = _queue.Count == 0;
            if (_queue.Count >= _capacity)
            {
                Remove(_queue.First!);
                Interlocked.Increment(ref _dropped);
                dropped = 1;
            }

            var node = _queue.AddLast(next);
            if (next is RosterReceived added)
            {
                _rosters[added.ChannelIndex] = node;
            }
        }
        if (wake)
        {
            _ready.Release();
        }
        return dropped;
    }

    internal bool TryRead(out SC2Event? next)
    {
        lock (_gate)
        {
            if (_queue.First is not { } first)
            {
                next = null;
                return false;
            }
            next = first.Value;
            Remove(first);
            return true;
        }
    }

    internal void Complete()
    {
        lock (_gate)
        {
            if (_completed)
            {
                return;
            }
            _completed = true;
        }
        _ready.Release(Math.Max(1, Volatile.Read(ref _waiters)));
    }

    internal async IAsyncEnumerable<SC2Event> ReadAllAsync(
        [EnumeratorCancellation] CancellationToken cancellation = default)
    {
        while (true)
        {
            while (TryRead(out var next))
            {
                yield return next!;
            }
            Interlocked.Increment(ref _waiters);
            bool finished;
            lock (_gate)
            {
                finished = _completed && _queue.Count == 0;
            }
            if (finished)
            {
                Interlocked.Decrement(ref _waiters);
                yield break;
            }
            try
            {
                await _ready.WaitAsync(cancellation).ConfigureAwait(false);
            }
            finally
            {
                Interlocked.Decrement(ref _waiters);
            }
        }
    }

    private void Remove(LinkedListNode<SC2Event> node)
    {
        if (node.Value is RosterReceived roster &&
            _rosters.TryGetValue(roster.ChannelIndex, out var registered) &&
            ReferenceEquals(node, registered))
        {
            _rosters.Remove(roster.ChannelIndex);
        }
        _queue.Remove(node);
    }
}
