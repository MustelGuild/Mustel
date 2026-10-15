using FurLab.Core.Constants;
using FurLab.Core.Interfaces;
using FurLab.Core.Models;
using Spectre.Console;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Database.PgPass.Remove;

/// <summary>
/// Removes a specific entry from the pgpass.conf file.
/// </summary>
public sealed class PgPassRemoveCommand : AsyncCommand<PgPassRemoveSettings>
{
    private readonly IPgPassService _pgPassService;

    /// <summary>
    /// Initializes a new instance of the <see cref="PgPassRemoveCommand"/> class.
    /// </summary>
    /// <param name="pgPassService">The pgpass service.</param>
    public PgPassRemoveCommand(IPgPassService pgPassService)
    {
        _pgPassService = pgPassService;
    }

    /// <inheritdoc/>
    protected override Task<int> ExecuteAsync(CommandContext context, PgPassRemoveSettings settings, CancellationToken cancellation)
    {
        if (string.IsNullOrWhiteSpace(settings.Database))
        {
            AnsiConsole.MarkupLine("[red]Error: the <database> argument is required.[/]");
            return Task.FromResult(2);
        }

        var key = new PgPassEntry
        {
            Hostname = settings.Host ?? FurLabConstants.DefaultHost,
            Port = settings.Port ?? FurLabConstants.DefaultPort,
            Database = settings.Database,
            Username = settings.Username ?? "postgres",
            Password = "placeholder"
        };

        var result = _pgPassService.RemoveEntry(key);

        if (result.Success)
        {
            AnsiConsole.MarkupLine($"[green]{result.Message}[/]");
            return Task.FromResult(0);
        }
        else if (result.Message.StartsWith("Entry not found"))
        {
            AnsiConsole.MarkupLine($"[yellow]{result.Message}[/]");
            return Task.FromResult(0);
        }
        else
        {
            AnsiConsole.MarkupLine($"[red]{result.Message}[/]");
            return Task.FromResult(1);
        }
    }
}
