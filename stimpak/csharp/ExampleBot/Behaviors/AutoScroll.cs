using Avalonia;
using Avalonia.Controls;

namespace ExampleBot.Behaviors;

/// <summary>
/// keeps a scroll viewer pinned to the bottom as content grows, unless the
/// reader has scrolled up — in which case they are reading, and yanking them
/// back would be rude.
/// </summary>
public static class AutoScroll
{
    private const double Slack = 8.0;

    public static readonly AttachedProperty<bool> EnabledProperty =
        AvaloniaProperty.RegisterAttached<ScrollViewer, bool>("Enabled", typeof(AutoScroll));

    public static void SetEnabled(ScrollViewer viewer, bool value) =>
        viewer.SetValue(EnabledProperty, value);

    public static bool GetEnabled(ScrollViewer viewer) => viewer.GetValue(EnabledProperty);

    static AutoScroll() =>
        EnabledProperty.Changed.AddClassHandler<ScrollViewer>((viewer, args) =>
        {
            if (args.GetNewValue<bool>())
            {
                viewer.PropertyChanged += OnViewerPropertyChanged;
            }
            else
            {
                viewer.PropertyChanged -= OnViewerPropertyChanged;
            }
        });

    private static void OnViewerPropertyChanged(object? sender, AvaloniaPropertyChangedEventArgs args)
    {
        if (sender is not ScrollViewer viewer || args.Property != ScrollViewer.ExtentProperty)
        {
            return;
        }
        // measure against the extent before this change, so a reader who was
        // already at the bottom stays there and one who was not is left alone.
        var previous = args.GetOldValue<Size>().Height;
        var wasAtEnd = previous <= viewer.Viewport.Height
            || viewer.Offset.Y + viewer.Viewport.Height >= previous - Slack;
        if (wasAtEnd)
        {
            viewer.ScrollToEnd();
        }
    }
}
