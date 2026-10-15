using FurLab.Core.Interfaces;
using Spectre.Console;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Settings.DbServers.Remove;

/// <summary>
/// Removes a configured database server.
/// Uses interactive selection when <c>--name</c> is not provided.
/// </summary>
public sealed class DbServersRemoveCommand : AsyncCommand<DbServersRemoveSettings>
{
    private readonly IUserConfigService _userConfigService;

    /// <summary>
    /// Initializes a new instance of the <see cref="DbServersRemoveCommand"/> class.
    /// </summary>
    /// <param name="userConfigService">The user configuration service.</param>
    public DbServersRemoveCommand(IUserConfigService userConfigService)
    {
        _userConfigService = userConfigService;
    }

    /// <inheritdoc/>
    protected override Task<int> ExecuteAsync(CommandContext context, DbServersRemoveSettings settings, CancellationToken cancellation)
    {
        var servers = _userConfigService.GetServers().ToList();

        if (servers.Count == 0)
        {
            AnsiConsole.MarkupLine("[yellow]No servers configured to remove.[/]");
            return Task.FromResult(0);
        }

        if (!string.IsNullOrWhiteSpace(settings.Name))
        {
            var removed = _userConfigService.RemoveServer(settings.Name);
            if (!removed)
            {
                throw new ArgumentException($"Server '{settings.Name}' not found.");
            }

            AnsiConsole.MarkupLine($"[green]Server '{settings.Name.EscapeMarkup()}' removed successfully.[/]");
            return Task.FromResult(0);
        }

        var selection = AnsiConsole.Prompt(
            new MultiSelectionPrompt<string>()
                .Title("Select servers to remove:")
                .PageSize(10)
                .AddChoices(servers.Select(s => s.Name)));

        if (selection.Count == 0)
        {
            AnsiConsole.MarkupLine("[yellow]No servers selected. Operation cancelled.[/]");
            return Task.FromResult(0);
        }

        foreach (var name in selection)
        {
            _userConfigService.RemoveServer(name);
            AnsiConsole.MarkupLine($"[green]Server '{name.EscapeMarkup()}' removed.[/]");
        }

        return Task.FromResult(0);
    }
}
