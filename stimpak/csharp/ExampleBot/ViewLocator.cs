using Avalonia.Controls;
using Avalonia.Controls.Templates;
using ExampleBot.ViewModels;

namespace ExampleBot;

/// <summary>maps a view model to its view by name, the avalonia convention.</summary>
public sealed class ViewLocator : IDataTemplate
{
    public Control Build(object? data)
    {
        if (data is null)
        {
            return new TextBlock { Text = "no view model" };
        }
        var name = data.GetType().FullName!.Replace("ViewModels", "Views", StringComparison.Ordinal)
            .Replace("ViewModel", "", StringComparison.Ordinal);
        var type = Type.GetType(name);
        return type is null
            ? new TextBlock { Text = $"no view for {name}" }
            : (Control)Activator.CreateInstance(type)!;
    }

    public bool Match(object? data) => data is ViewModelBase;
}
