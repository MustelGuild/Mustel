using FurLab.Core.Interfaces;
using Spectre.Console;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Settings.DbServers.SetPassword;

/// <summary>
/// Defines or redefines the encrypted password for a configured database server.
/// </summary>
public sealed class DbServersSetPasswordCommand : AsyncCommand<DbServersSetPasswordSettings>
{
    private readonly IUserConfigService _userConfigService;
    private readonly ICredentialService _credentialService;
    private readonly IPostgresPasswordHandler _postgresPasswordHandler;

    /// <summary>
    /// Initializes a new instance of the <see cref="DbServersSetPasswordCommand"/> class.
    /// </summary>
    /// <param name="userConfigService">The user configuration service.</param>
    /// <param name="credentialService">The credential service for encrypting passwords.</param>
    /// <param name="postgresPasswordHandler">The password handler for interactive password input.</param>
    public DbServersSetPasswordCommand(
        IUserConfigService userConfigService,
        ICredentialService credentialService,
        IPostgresPasswordHandler postgresPasswordHandler)
    {
        _userConfigService = userConfigService;
        _credentialService = credentialService;
        _postgresPasswordHandler = postgresPasswordHandler;
    }

    /// <inheritdoc/>
    protected override Task<int> ExecuteAsync(CommandContext context, DbServersSetPasswordSettings settings, CancellationToken cancellation)
    {
        var name = settings.Name;

        if (string.IsNullOrWhiteSpace(name))
        {
            name = SelectServer("Select a server to set the password:");
            if (name == null)
            {
                return Task.FromResult(0);
            }
        }

        SavePasswordForServer(name);
        return Task.FromResult(0);
    }

    private void SavePasswordForServer(string serverName)
    {
        var server = _userConfigService.GetServer(serverName);
        if (server == null)
        {
            throw new ArgumentException($"Server '{serverName}' not found.");
        }

        var password = _postgresPasswordHandler.ReadPasswordInteractively("Password: ");
        var encrypted = _credentialService.Encrypt(password);
        _userConfigService.SetEncryptedPassword(serverName, encrypted);
        AnsiConsole.MarkupLine($"[green]Password saved securely for '{serverName.EscapeMarkup()}'.[/]");
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
