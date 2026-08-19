using Avalonia;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Markup.Xaml;
using ExampleBot.Services;
using ExampleBot.ViewModels;
using ExampleBot.Views;

namespace ExampleBot;

public partial class App : Application
{
    public override void Initialize() => AvaloniaXamlLoader.Load(this);

    public override void OnFrameworkInitializationCompleted()
    {
        if (ApplicationLifetime is IClassicDesktopStyleApplicationLifetime desktop)
        {
            // composition happens here and nowhere else: the window knows a
            // view model, the view model knows a session interface, and only
            // this line knows which session is the real one.
            var session = new StimpakSession("bot-credentials.bin");
            var model = new MainWindowViewModel(session);
            desktop.MainWindow = new MainWindow { DataContext = model };
            model.Start();
        }
        base.OnFrameworkInitializationCompleted();
    }
}
