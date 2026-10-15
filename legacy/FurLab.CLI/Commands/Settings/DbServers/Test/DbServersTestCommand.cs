using FurLab.Core.Interfaces;
using FurLab.Core.Models;
using Spectre.Console;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Settings.DbServers.Test;

/// <summary>
/// Tests the connection to a configured database server.
/// </summary>
public sealed class DbServersTestCommand : AsyncCommand<DbServersTestSettings>
{
    private readonly IUserConfigService _userConfigService;
    private readonly ICredentialService _credentialService;
    private readonly IDatabaseService _databaseService;

    /// <summary>
    /// Initializes a new instance of the <see cref="DbServersTestCommand"/> class.
    /// </summary>
    /// <param name="userConfigService">The user configuration service.</param>
    /// <param name="credentialService">The credential service for decrypting passwords.</param>
    /// <param name="databaseService">The database service for connection testing.</param>
    public DbServersTestCommand(
        IUserConfigService userConfigService,
        ICredentialService credentialService,
        IDatabaseService databaseService)
    {
        _userConfigService = userConfigService;
        _credentialService = credentialService;
        _databaseService = databaseService;
    }

    /// <inheritdoc/>
    protected override async Task<int> ExecuteAsync(CommandContext context, DbServersTestSettings settings, CancellationToken cancellation)
    {
        var name = settings.Name;

        if (string.IsNullOrWhiteSpace(name))
        {
            name = SelectServer("Select a server to test:");
            if (name == null)
            {
                return 0;
            }
        }

        var server = _userConfigService.GetServer(name);
        if (server == null)
        {
            throw new ArgumentException($"Server '{name}' not found.");
        }

        AnsiConsole.MarkupLine($"[cyan]Testing connection to {server.Name.EscapeMarkup()} ({server.Host.EscapeMarkup()}:{server.Port})...[/]");

        var password = _credentialService.TryDecrypt(server.EncryptedPassword);
        if (password == null)
        {
            AnsiConsole.MarkupLine($"[yellow]No password found for '{server.Name.EscapeMarkup()}'. Type to test (will not be saved):[/]");
            password = AnsiConsole.Prompt(
                new TextPrompt<string>("Password: ")
                    .Secret());
        }

        try
        {
            var connectionOptions = new DatabaseConnectionOptions
            {
                Host = server.Host,
                Port = server.Port.ToString(),
                Username = server.Username,
                Password = password,
                SslMode = server.SslMode
            };

            var databases = await _databaseService.ListDatabasesAsync(connectionOptions, cancellation);

            AnsiConsole.MarkupLine($"[green]Connection to {server.Host.EscapeMarkup()}:{server.Port} successful[/]");
            AnsiConsole.MarkupLine($"[green]Authenticated as {server.Username.EscapeMarkup()}[/]");

            if (databases.Count > 0)
            {
                AnsiConsole.MarkupLine($"[green]Databases found: {string.Join(", ", databases)}[/]");
            }
            else
            {
                AnsiConsole.MarkupLine("[yellow]No accessible databases found[/]");
            }
        }
        catch (InvalidOperationException ex) when (
            ex.Message.Contains("authentication", StringComparison.OrdinalIgnoreCase) ||
            ex.Message.Contains("password", StringComparison.OrdinalIgnoreCase))
        {
            throw new InvalidOperationException($"Authentication failed: {ex.Message}", ex);
        }
        catch (Exception ex) when (ex is not ArgumentException)
        {
            throw new InvalidOperationException($"Connection failed: {ex.Message}", ex);
        }

        return 0;
    }

    private string? SelectServer(string prompt)
    {
        var servers = _userConfigService.GetServers();

        if (servers.Count == 0)
        {
            AnsiConsole.MarkupLine("[yellow]No servers configured. Use 'fur settings db-servers add' to add one.[/]");
            return null;
        }

        return AnsiConsole.Prompt(
            new SelectionPrompt<string>()
                .Title(prompt)
                .PageSize(10)
                .AddChoices(servers.Select(s => s.Name)));
    }
}
