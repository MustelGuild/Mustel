using FurLab.Core.Interfaces;
using FurLab.Core.Models;
using Spectre.Console;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Settings.DbServers.Add;

/// <summary>
/// Adds a new database server configuration.
/// Enters interactive mode when <c>--name</c> or <c>--host</c> are not provided.
/// </summary>
public sealed class DbServersAddCommand : AsyncCommand<DbServersAddSettings>
{
    private readonly IUserConfigService _userConfigService;
    private readonly ICredentialService _credentialService;
    private readonly IPostgresPasswordHandler _postgresPasswordHandler;

    /// <summary>
    /// Initializes a new instance of the <see cref="DbServersAddCommand"/> class.
    /// </summary>
    /// <param name="userConfigService">The user configuration service.</param>
    /// <param name="credentialService">The credential service for encrypting passwords.</param>
    /// <param name="postgresPasswordHandler">The password handler for interactive password input.</param>
    public DbServersAddCommand(
        IUserConfigService userConfigService,
        ICredentialService credentialService,
        IPostgresPasswordHandler postgresPasswordHandler)
    {
        _userConfigService = userConfigService;
        _credentialService = credentialService;
        _postgresPasswordHandler = postgresPasswordHandler;
    }

    /// <inheritdoc/>
    protected override Task<int> ExecuteAsync(CommandContext context, DbServersAddSettings settings, CancellationToken cancellation)
    {
        if (string.IsNullOrWhiteSpace(settings.Name) || string.IsNullOrWhiteSpace(settings.Host))
        {
            return Task.FromResult(AddServerInteractive(settings));
        }

        return Task.FromResult(AddServerDirect(settings));
    }

    private int AddServerDirect(DbServersAddSettings settings)
    {
        if (string.IsNullOrWhiteSpace(settings.Name))
        {
            throw new ArgumentException("--name (-n) is required.");
        }

        if (string.IsNullOrWhiteSpace(settings.Host))
        {
            throw new ArgumentException("--host (-h) is required.");
        }

        if (!SecurityUtils.IsValidHost(settings.Host))
        {
            throw new ArgumentException($"Invalid host: '{settings.Host}'.");
        }

        if (settings.Port is < 1 or > 65535)
        {
            throw new ArgumentException($"Invalid port: {settings.Port}. Must be between 1 and 65535.");
        }

        if (!string.IsNullOrWhiteSpace(settings.Username) && !SecurityUtils.IsValidUsername(settings.Username))
        {
            throw new ArgumentException($"Invalid username: '{settings.Username}'.");
        }

        var existing = _userConfigService.GetServer(settings.Name);
        if (existing != null)
        {
            throw new InvalidOperationException($"Server '{settings.Name}' already exists. Use a different name or remove the existing one first.");
        }

        var server = BuildServerFromSettings(settings);
        _userConfigService.AddOrUpdateServer(server);
        AnsiConsole.MarkupLine($"[green]Server '{server.Name.EscapeMarkup()}' added successfully.[/]");
        AnsiConsole.MarkupLine($"[dim]Use 'fur settings db-servers set-password {server.Name.EscapeMarkup()}' to set the password.[/]");
        return 0;
    }

    private int AddServerInteractive(DbServersAddSettings settings)
    {
        var name = AnsiConsole.Ask("Server name*: ", settings.Name ?? string.Empty);
        if (string.IsNullOrWhiteSpace(name))
        {
            AnsiConsole.MarkupLine("[red]Server name is required.[/]");
            return 1;
        }

        var host = AnsiConsole.Ask("Host*: ", settings.Host ?? string.Empty);
        if (string.IsNullOrWhiteSpace(host))
        {
            AnsiConsole.MarkupLine("[red]Host is required.[/]");
            return 1;
        }

        if (!SecurityUtils.IsValidHost(host))
        {
            AnsiConsole.MarkupLine($"[red]Invalid host: '{host.EscapeMarkup()}'.[/]");
            return 1;
        }

        var port = AnsiConsole.Ask("Port [[5432]]: ", settings.Port > 0 ? settings.Port : 5432);
        if (port is < 1 or > 65535)
        {
            AnsiConsole.MarkupLine($"[red]Invalid port: {port}. Must be between 1 and 65535.[/]");
            return 1;
        }

        var defaultUsername = !string.IsNullOrWhiteSpace(settings.Username) ? settings.Username : "postgres";
        var username = AnsiConsole.Ask("Username [[postgres]]: ", defaultUsername);
        if (!SecurityUtils.IsValidUsername(username))
        {
            AnsiConsole.MarkupLine($"[red]Invalid username: '{username.EscapeMarkup()}'.[/]");
            return 1;
        }

        var databasesInput = AnsiConsole.Ask<string?>("Databases (comma-separated, optional): ", string.Empty);
        var databases = ParseDatabases(databasesInput);

        var fetchAll = AnsiConsole.Confirm("Auto-discover databases? ", false);
        List<string> excludePatterns = ["template*", "postgres"];
        if (fetchAll)
        {
            var patternsInput = AnsiConsole.Ask<string?>("Exclude patterns (comma-separated) [[template*,postgres]]: ", "template*,postgres");
            excludePatterns = ParseExcludePatterns(patternsInput);
        }

        var sslMode = AnsiConsole.Prompt(
            new SelectionPrompt<string>()
                .Title("SSL Mode [[Prefer]]: ")
                .AddChoices("Disable", "Allow", "Prefer", "Require", "VerifyCA", "VerifyFull"));

        var timeout = AnsiConsole.Ask("Connection timeout [[30s]]: ", 30);
        var maxParallelism = AnsiConsole.Ask("Max Parallelism [[4]]: ", 4);

        var server = new ServerConfigEntry
        {
            Name = name,
            Host = host,
            Port = port,
            Username = username,
            Databases = databases,
            FetchAllDatabases = fetchAll,
            ExcludePatterns = excludePatterns,
            SslMode = sslMode,
            Timeout = timeout,
            MaxParallelism = maxParallelism
        };

        var existing = _userConfigService.GetServer(name);
        if (existing != null)
        {
            throw new InvalidOperationException($"Server '{name}' already exists.");
        }

        var action = AnsiConsole.Prompt(
            new SelectionPrompt<string>()
                .Title("Action:")
                .AddChoices("Save and set password", "Save and test connection", "Save without password", "Cancel"));

        if (action == "Cancel")
        {
            AnsiConsole.MarkupLine("[yellow]Operation cancelled.[/]");
            return 0;
        }

        _userConfigService.AddOrUpdateServer(server);

        if (action == "Save and set password")
        {
            SavePasswordForServer(name);
        }
        else if (action == "Save and test connection")
        {
            AnsiConsole.MarkupLine($"[green]Server '{name.EscapeMarkup()}' added successfully.[/]");
            AnsiConsole.MarkupLine($"[dim]Run 'fur settings db-servers test {name.EscapeMarkup()}' to test the connection.[/]");
        }
        else
        {
            AnsiConsole.MarkupLine($"[green]Server '{name.EscapeMarkup()}' added successfully.[/]");
            AnsiConsole.MarkupLine($"[dim]Use 'fur settings db-servers set-password {name.EscapeMarkup()}' to set the password.[/]");
        }

        return 0;
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

    private static ServerConfigEntry BuildServerFromSettings(DbServersAddSettings settings)
    {
        var databases = ParseDatabases(settings.Databases);
        var excludePatterns = ParseExcludePatterns(settings.ExcludePatterns);

        return new ServerConfigEntry
        {
            Name = settings.Name ?? string.Empty,
            Host = settings.Host ?? string.Empty,
            Port = settings.Port,
            Username = settings.Username ?? string.Empty,
            Databases = databases,
            FetchAllDatabases = settings.FetchAllDatabases,
            ExcludePatterns = excludePatterns,
            SslMode = settings.SslMode ?? "Prefer",
            Timeout = settings.Timeout ?? 30,
            MaxParallelism = settings.MaxParallelism ?? 4
        };
    }

    private static List<string> ParseDatabases(string? input)
    {
        if (string.IsNullOrWhiteSpace(input))
        {
            return [];
        }

        return input.Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
            .Where(db => !string.IsNullOrWhiteSpace(db))
            .ToList();
    }

    private static List<string> ParseExcludePatterns(string? input)
    {
        if (string.IsNullOrWhiteSpace(input))
        {
            return ["template*", "postgres"];
        }

        return input.Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
            .Where(p => !string.IsNullOrWhiteSpace(p))
            .ToList();
    }
}
