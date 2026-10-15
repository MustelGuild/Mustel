using FurLab.Core.Interfaces;
using Spectre.Console;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Settings.DbServers.List;

/// <summary>
/// Lists all configured database servers.
/// </summary>
public sealed class DbServersListCommand : AsyncCommand<DbServersListSettings>
{
    private readonly IUserConfigService _userConfigService;

    /// <summary>
    /// Initializes a new instance of the <see cref="DbServersListCommand"/> class.
    /// </summary>
    /// <param name="userConfigService">The user configuration service.</param>
    public DbServersListCommand(IUserConfigService userConfigService)
    {
        _userConfigService = userConfigService;
    }

    /// <inheritdoc/>
    protected override Task<int> ExecuteAsync(CommandContext context, DbServersListSettings settings, CancellationToken cancellation)
    {
        var servers = _userConfigService.GetServers();

        if (servers.Count == 0)
        {
            AnsiConsole.MarkupLine("[yellow]No servers configured. Use 'fur settings db-servers add' to add one.[/]");
            return Task.FromResult(0);
        }

        var table = new Table();
        table.AddColumn("Name");
        table.AddColumn("Host");
        table.AddColumn("Port");
        table.AddColumn("Username");
        table.AddColumn("Databases");
        table.AddColumn("Auto-DB");

        foreach (var server in servers)
        {
            var dbInfo = server.FetchAllDatabases
                ? "[green](auto)[/]"
                : (server.Databases.Count > 0 ? string.Join(", ", server.Databases) : "[dim](default)[/]");

            table.AddRow(
                server.Name.EscapeMarkup(),
                server.Host.EscapeMarkup(),
                server.Port.ToString(),
                server.Username.EscapeMarkup(),
                dbInfo,
                server.FetchAllDatabases ? "[green]Yes[/]" : "[dim]No[/]");
        }

        AnsiConsole.Write(table);
        return Task.FromResult(0);
    }
}
