using Avalonia.Controls;
using Avalonia.Markup.Xaml;
using ExampleBot.ViewModels;

namespace ExampleBot.Views;

/// <summary>
/// layout only. everything the window shows or does lives in
/// <see cref="MainWindowViewModel"/>, so the session can be swapped for a
/// sample one and the whole thing still behaves.
/// </summary>
public partial class MainWindow : Window
{
    public MainWindow() => AvaloniaXamlLoader.Load(this);

    protected override void OnClosed(EventArgs args)
    {
        (DataContext as IDisposable)?.Dispose();
        base.OnClosed(args);
    }
}
