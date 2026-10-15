using FurLab.Core.Interfaces;
using Spectre.Console;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Database.PgPass.List;

/// <summary>
/// Lists all entries in the pgpass.conf file with masked passwords.
/// </summary>
public sealed class PgPassListCommand : AsyncCommand<PgPassListSettings>
{
    private readonly IPgPassService _pgPassService;

    /// <summary>
    /// Initializes a new instance of the <see cref="PgPassListCommand"/> class.
    /// </summary>
    /// <param name="pgPassService">The pgpass service.</param>
    public PgPassListCommand(IPgPassService pgPassService)
    {
        _pgPassService = pgPassService;
    }

    /// <inheritdoc/>
    protected override Task<int> ExecuteAsync(CommandContext context, PgPassListSettings settings, CancellationToken cancellation)
    {
        var entries = _pgPassService.ListEntries().ToList();

        if (entries.Count == 0)
        {
            AnsiConsole.WriteLine("No entries configured in pgpass.conf.");
            return Task.FromResult(0);
        }

        var table = new Table()
            .Border(TableBorder.Square)
            .Title("[bold]pgpass.conf[/]")
            .AddColumn(new TableColumn("[bold]HOSTNAME[/]"))
            .AddColumn(new TableColumn("[bold]PORT[/]"))
            .AddColumn(new TableColumn("[bold]DATABASE[/]"))
            .AddColumn(new TableColumn("[bold]USERNAME[/]"))
            .AddColumn(new TableColumn("[bold]PASSWORD[/]"));

        foreach (var entry in entries)
        {
            table.AddRow(
                entry.Hostname,
                entry.Port,
                entry.Database,
                entry.Username,
                "****");
        }

        AnsiConsole.Write(table);

        return Task.FromResult(0);
    }
}
