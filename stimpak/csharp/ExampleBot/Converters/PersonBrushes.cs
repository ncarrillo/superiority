using System.Globalization;
using Avalonia.Data.Converters;
using Avalonia.Media;
using Stimpak;

namespace ExampleBot.Converters;

/// <summary>
/// how a person looks. mapping a model value to a brush is a view concern, and
/// keeping it here is what lets the roster bind straight to
/// <see cref="Person"/> instead of wrapping every one in a view model.
/// </summary>
public sealed class PresenceAccent : IValueConverter
{
    private static readonly IBrush Text = Brush.Parse("#d6e0f0");
    private static readonly IBrush Clan = Brush.Parse("#f0aa64");

    public object Convert(object? value, Type type, object? parameter, CultureInfo culture) =>
        value is Person { ClanTag: not null } ? Clan : Text;

    public object ConvertBack(object? value, Type type, object? parameter, CultureInfo culture) =>
        throw new NotSupportedException();
}

/// <summary>people who are not really there recede rather than disappear.</summary>
public sealed class PresenceOpacity : IValueConverter
{
    public object Convert(object? value, Type type, object? parameter, CultureInfo culture) =>
        value is Presence.Offline or Presence.Away ? 0.45 : 1.0;

    public object ConvertBack(object? value, Type type, object? parameter, CultureInfo culture) =>
        throw new NotSupportedException();
}

public sealed class PresenceLabel : IValueConverter
{
    public object Convert(object? value, Type type, object? parameter, CultureInfo culture) =>
        value switch
        {
            Presence.Available => "Available",
            Presence.Away => "Away",
            Presence.Busy => "Busy",
            Presence.InGame => "In game",
            Presence.Offline => "Offline",
            _ => "Unknown",
        };

    public object ConvertBack(object? value, Type type, object? parameter, CultureInfo culture) =>
        throw new NotSupportedException();
}
