using FurLab.Core.Constants;
using FurLab.Core.Interfaces;
using FurLab.Core.Models;
using Spectre.Console;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Database.PgPass.Add;

/// <summary>
/// Adds a new PostgreSQL credential entry to the pgpass.conf file.
/// </summary>
public sealed class PgPassAddCommand : AsyncCommand<PgPassAddSettings>
{
    private readonly IPgPassService _pgPassService;
    private readonly IPostgresPasswordHandler _passwordHandler;

    /// <summary>
    /// Initializes a new instance of the <see cref="PgPassAddCommand"/> class.
    /// </summary>
    /// <param name="pgPassService">The pgpass service.</param>
    /// <param name="passwordHandler">The PostgreSQL password handler.</param>
    public PgPassAddCommand(IPgPassService pgPassService, IPostgresPasswordHandler passwordHandler)
    {
        _pgPassService = pgPassService;
        _passwordHandler = passwordHandler;
    }

    /// <inheritdoc/>
    protected override Task<int> ExecuteAsync(CommandContext context, PgPassAddSettings settings, CancellationToken cancellation)
    {
        if (string.IsNullOrWhiteSpace(settings.Database))
        {
            AnsiConsole.MarkupLine("[red]Error: the --database argument is required.[/]");
            return Task.FromResult(2);
        }

        var host = settings.Host ?? FurLabConstants.DefaultHost;
        if (!SecurityUtils.IsValidHost(host))
        {
            AnsiConsole.MarkupLine($"[red]Error: invalid host format: \"{host}\".[/]");
            return Task.FromResult(2);
        }

        var port = settings.Port ?? FurLabConstants.DefaultPort;
        if (!SecurityUtils.IsValidPort(port))
        {
            AnsiConsole.MarkupLine("[red]Error: port must be a number between 1 and 65535.[/]");
            return Task.FromResult(2);
        }

        var username = settings.Username ?? "postgres";

        var password = settings.Password;
        if (string.IsNullOrEmpty(password))
        {
            password = _passwordHandler.ReadPasswordInteractively("Password: ");
        }

        if (string.IsNullOrEmpty(password))
        {
            AnsiConsole.MarkupLine("[red]Error: password cannot be empty.[/]");
            return Task.FromResult(2);
        }

        var entry = new PgPassEntry
        {
            Hostname = host,
            Port = port,
            Database = settings.Database,
            Username = username,
            Password = password
        };

        var result = _pgPassService.AddEntry(entry);

        if (result.Success)
        {
            AnsiConsole.MarkupLine($"[green]{result.Message}[/]");
            return Task.FromResult(0);
        }
        else
        {
            AnsiConsole.MarkupLine($"[red]{result.Message}[/]");
            return Task.FromResult(1);
        }
    }
}
