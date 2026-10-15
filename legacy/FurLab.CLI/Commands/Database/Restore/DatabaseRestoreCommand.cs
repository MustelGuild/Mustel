using System.Diagnostics;
using FurLab.CLI.Exceptions.Database;
using FurLab.CLI.Exceptions.File;
using FurLab.CLI.Exceptions.Security;
using FurLab.Core.Constants;
using FurLab.Core.Interfaces;
using Microsoft.Extensions.Logging;
using Spectre.Console;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Database.Restore;

/// <summary>
/// Restores a PostgreSQL database from a backup using pg_restore.
/// </summary>
public sealed class DatabaseRestoreCommand : AsyncCommand<DatabaseRestoreSettings>
{
    private readonly IPostgresBinaryLocator _binaryLocator;
    private readonly IPostgresPasswordHandler _passwordHandler;
    private readonly IUserConfigService _userConfigService;
    private readonly ICredentialService _credentialService;
    private readonly ILogger<DatabaseRestoreCommand> _logger;

    /// <summary>
    /// Initializes a new instance of the <see cref="DatabaseRestoreCommand"/> class.
    /// </summary>
    /// <param name="binaryLocator">The PostgreSQL binary locator service.</param>
    /// <param name="passwordHandler">The PostgreSQL password handler service.</param>
    /// <param name="userConfigService">The user configuration service.</param>
    /// <param name="credentialService">The credential encryption service.</param>
    /// <param name="logger">The logger instance.</param>
    public DatabaseRestoreCommand(
        IPostgresBinaryLocator binaryLocator,
        IPostgresPasswordHandler passwordHandler,
        IUserConfigService userConfigService,
        ICredentialService credentialService,
        ILogger<DatabaseRestoreCommand> logger)
    {
        _binaryLocator = binaryLocator;
        _passwordHandler = passwordHandler;
        _userConfigService = userConfigService;
        _credentialService = credentialService;
        _logger = logger;
    }

    /// <inheritdoc/>
    protected override Task<int> ExecuteAsync(CommandContext context, DatabaseRestoreSettings settings, CancellationToken cancellation)
    {
        var (nullableConfig, errorCode) = LoadAndValidateDatabaseRestoreConfiguration(settings);

        if (nullableConfig is not { } config)
        {
            return Task.FromResult(errorCode);
        }

        try
        {
            if (config.RestoreAll)
            {
                RestoreAllDatabases(config, settings);
            }
            else
            {
                RestoreSingleDatabase(config, settings);
            }

            return Task.FromResult(0);
        }
        catch (PostgresBinaryNotFoundException ex)
        {
            AnsiConsole.MarkupLine($"[red]Error:[/] {ex.Message}");
            return Task.FromResult(1);
        }
        catch (RestoreFailedException ex)
        {
            AnsiConsole.MarkupLine($"[red]Error:[/] {ex.Message}");
            return Task.FromResult(1);
        }
        catch (PathTraversalException ex)
        {
            AnsiConsole.MarkupLine($"[red]Error:[/] {ex.Message}");
            return Task.FromResult(2);
        }
        catch (FurLabFileNotFoundException ex)
        {
            AnsiConsole.MarkupLine($"[red]Error:[/] {ex.Message}");
            return Task.FromResult(2);
        }
        catch (Exception ex)
        {
            AnsiConsole.MarkupLine($"[red]Error:[/] {ex.Message}");
            return Task.FromResult(1);
        }
    }

    /// <summary>
    /// Loads and validates the restore configuration from settings and service defaults.
    /// </summary>
    /// <param name="settings">The command settings.</param>
    /// <returns>A tuple containing the validated config or null, and an error code if validation failed.</returns>
    private (DatabaseRestoreConfig? Config, int ErrorCode) LoadAndValidateDatabaseRestoreConfiguration(DatabaseRestoreSettings settings)
    {
        var host = settings.Host ?? FurLabConstants.DefaultHost;
        var port = settings.Port ?? FurLabConstants.DefaultPort;
        var username = settings.Username;
        var password = settings.Password ?? ResolvePasswordFromUserConfig(host, port);

        if (!SecurityUtils.IsValidHost(host))
        {
            AnsiConsole.MarkupLine($"[red]Error:[/] Invalid host: '{host}'");
            return (null, 2);
        }

        if (!SecurityUtils.IsValidPort(port))
        {
            AnsiConsole.MarkupLine($"[red]Error:[/] Invalid port: '{port}'. Port must be between 1 and 65535.");
            return (null, 2);
        }

        if (!string.IsNullOrWhiteSpace(username) && !SecurityUtils.IsValidUsername(username))
        {
            AnsiConsole.MarkupLine($"[red]Error:[/] Invalid username: '{username}'");
            return (null, 2);
        }

        var databaseName = settings.Database ?? string.Empty;

        if (!settings.All && string.IsNullOrWhiteSpace(databaseName))
        {
            AnsiConsole.MarkupLine("[red]Error:[/] Database name is required when --all is not specified.");
            return (null, 2);
        }

        if (!string.IsNullOrWhiteSpace(settings.SslMode))
        {
            var validSslModes = new[] { "Disable", "Prefer", "Require", "VerifyCA", "VerifyFull" };
            if (!validSslModes.Contains(settings.SslMode, StringComparer.OrdinalIgnoreCase))
            {
                AnsiConsole.MarkupLine($"[red]Error:[/] Invalid SSL mode: '{settings.SslMode}'. Valid values: Disable, Prefer, Require, VerifyCA, VerifyFull.");
                return (null, 2);
            }
        }

        if (settings.Timeout.HasValue && settings.Timeout.Value <= 0)
        {
            AnsiConsole.MarkupLine("[red]Error:[/] Timeout must be a positive integer.");
            return (null, 2);
        }

        if (settings.CommandTimeout.HasValue && settings.CommandTimeout.Value <= 0)
        {
            AnsiConsole.MarkupLine("[red]Error:[/] Command timeout must be a positive integer.");
            return (null, 2);
        }

        var config = new DatabaseRestoreConfig
        {
            Host = host,
            Port = port,
            Username = username,
            Password = password,
            DatabaseName = databaseName,
            RestoreAll = settings.All,
            InputFile = settings.InputFile,
            OutputPath = settings.Directory,
            SslMode = settings.SslMode,
            Timeout = settings.Timeout,
            CommandTimeout = settings.CommandTimeout
        };

        return ((DatabaseRestoreConfig?)config, 0);
    }

    /// <summary>
    /// Resolves the database password from the user configuration store.
    /// </summary>
    /// <param name="host">The database host.</param>
    /// <param name="port">The database port.</param>
    /// <returns>The decrypted password, or null if not found.</returns>
    private string? ResolvePasswordFromUserConfig(string host, string port)
    {
        try
        {
            var servers = _userConfigService.GetServers();
            var matchingServer = servers.FirstOrDefault(s =>
                string.Equals(s.Host, host, StringComparison.OrdinalIgnoreCase) &&
                s.Port.ToString() == port);

            if (matchingServer?.EncryptedPassword != null)
            {
                return _credentialService.TryDecrypt(matchingServer.EncryptedPassword);
            }
        }
        catch
        {
        }

        return null;
    }

    /// <summary>
    /// Gets the password, prompting interactively if not provided.
    /// </summary>
    /// <param name="password">The password or null.</param>
    /// <returns>The resolved password.</returns>
    private string GetPassword(string? password)
    {
        if (string.IsNullOrWhiteSpace(password))
        {
            return _passwordHandler.ReadPasswordInteractively("Enter password: ");
        }

        return password;
    }

    /// <summary>
    /// Restores a single database from a dump file.
    /// </summary>
    /// <param name="config">The validated restore configuration.</param>
    /// <param name="settings">The command settings.</param>
    private void RestoreSingleDatabase(DatabaseRestoreConfig config, DatabaseRestoreSettings settings)
    {
        var password = GetPassword(config.Password);

        var inputPath = config.InputFile;
        if (string.IsNullOrWhiteSpace(inputPath))
        {
            inputPath = $"{config.DatabaseName}{FurLabConstants.DumpExtension}";
        }

        var fullPath = Path.GetFullPath(inputPath);
        if (!SecurityUtils.IsValidPath(fullPath))
        {
            throw new PathTraversalException(inputPath);
        }

        if (!File.Exists(fullPath))
        {
            throw new FurLabFileNotFoundException(fullPath);
        }

        _logger.LogInformation("Restoring database '{DatabaseName}'...", config.DatabaseName);
        _logger.LogInformation("Host: {Host}:{Port}", config.Host, config.Port);
        _logger.LogInformation("Username: {Username}", config.Username ?? "N/A");
        _logger.LogInformation("Input: {FullPath}", Path.GetFullPath(fullPath));

        var pgRestorePath = _binaryLocator.FindPgRestore();
        if (pgRestorePath == null)
        {
            throw new PostgresBinaryNotFoundException(FurLabConstants.PgRestoreExecutable);
        }

        CreateDatabaseIfNeeded(config.Host, config.Port, config.Username ?? string.Empty, password, config.DatabaseName);

        var arguments = BuildPgRestoreArguments(config, settings, fullPath);

        var startInfo = new ProcessStartInfo
        {
            FileName = pgRestorePath,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
            CreateNoWindow = true
        };
        foreach (var arg in arguments)
        {
            startInfo.ArgumentList.Add(arg);
        }

        startInfo.Environment["PGPASSWORD"] = password;

        if (!string.IsNullOrWhiteSpace(config.SslMode))
        {
            startInfo.Environment["PGSSLMODE"] = config.SslMode;
        }

        if (config.Timeout.HasValue)
        {
            startInfo.Environment["PGCONNECT_TIMEOUT"] = config.Timeout.Value.ToString();
        }

        if (config.CommandTimeout.HasValue)
        {
            startInfo.Environment["PGCOMMAND_TIMEOUT"] = config.CommandTimeout.Value.ToString();
        }

        try
        {
            using var process = Process.Start(startInfo);
            if (process == null)
            {
                throw new Exception("Failed to start pg_restore process.");
            }

            var output = process.StandardOutput.ReadToEnd();
            var error = process.StandardError.ReadToEnd();

            process.WaitForExit();

            if (process.ExitCode != 0)
            {
                throw new RestoreFailedException($"pg_restore failed with exit code {process.ExitCode}. Error: {error}");
            }

            _logger.LogInformation("Database '{DatabaseName}' restored successfully from: {FullPath}", config.DatabaseName, Path.GetFullPath(fullPath));

            if (!string.IsNullOrWhiteSpace(output))
            {
                _logger.LogDebug("{Output}", output);
            }
        }
        finally
        {
            startInfo.Environment["PGPASSWORD"] = string.Empty;
        }
    }

    /// <summary>
    /// Restores all databases from dump files in a directory.
    /// </summary>
    /// <param name="config">The validated restore configuration.</param>
    /// <param name="settings">The command settings.</param>
    private void RestoreAllDatabases(DatabaseRestoreConfig config, DatabaseRestoreSettings settings)
    {
        var password = GetPassword(config.Password);

        var inputDirectory = config.OutputPath;
        if (string.IsNullOrWhiteSpace(inputDirectory))
        {
            inputDirectory = Directory.GetCurrentDirectory();
        }

        if (!Directory.Exists(inputDirectory))
        {
            throw new DirectoryNotFoundException($"Directory not found: {Path.GetFullPath(inputDirectory)}");
        }

        _logger.LogInformation("Scanning for {Extension} files in: {FullPath}", FurLabConstants.DumpExtension, Path.GetFullPath(inputDirectory));

        var dumpFiles = Directory.GetFiles(inputDirectory, $"*{FurLabConstants.DumpExtension}", SearchOption.TopDirectoryOnly);

        if (dumpFiles.Length == 0)
        {
            _logger.LogWarning("No {Extension} files found in the directory.", FurLabConstants.DumpExtension);
            return;
        }

        _logger.LogInformation("Found {Count} {Extension} files:", dumpFiles.Length, FurLabConstants.DumpExtension);
        foreach (var file in dumpFiles)
        {
            _logger.LogInformation("  - {File}", Path.GetFileName(file));
        }

        _logger.LogInformation(string.Empty);

        var successCount = 0;
        var failureCount = 0;

        foreach (var dumpFile in dumpFiles)
        {
            var databaseName = Path.GetFileNameWithoutExtension(dumpFile);

            if (!SecurityUtils.IsValidPostgreSQLIdentifier(databaseName))
            {
                _logger.LogWarning("Skipping invalid database name: '{Database}'", databaseName);
                failureCount++;
                continue;
            }

            _logger.LogInformation("Restoring database '{Database}'...", databaseName);

            try
            {
                var singleConfig = config with
                {
                    DatabaseName = databaseName,
                    InputFile = dumpFile,
                    Password = password
                };

                RestoreSingleDatabaseInternal(singleConfig, settings);
                _logger.LogInformation("Database restored successfully: {Database}", databaseName);
                successCount++;
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Failed to restore database '{Database}': {Message}", databaseName, ex.Message);
                failureCount++;
            }

            _logger.LogInformation(string.Empty);
        }

        PrintRestoreSummary(successCount, failureCount, dumpFiles.Length, inputDirectory);
    }

    /// <summary>
    /// Restores a single database from a dump file without interactive output.
    /// Used internally by the bulk restore operation.
    /// </summary>
    /// <param name="config">The validated restore configuration.</param>
    /// <param name="settings">The command settings.</param>
    private void RestoreSingleDatabaseInternal(DatabaseRestoreConfig config, DatabaseRestoreSettings settings)
    {
        var pgRestorePath = _binaryLocator.FindPgRestore();
        if (pgRestorePath == null)
        {
            throw new PostgresBinaryNotFoundException(FurLabConstants.PgRestoreExecutable);
        }

        CreateDatabaseIfNeeded(config.Host, config.Port, config.Username ?? string.Empty, config.Password ?? string.Empty, config.DatabaseName);

        var arguments = BuildPgRestoreArguments(config, settings, config.InputFile ?? string.Empty);

        var startInfo = new ProcessStartInfo
        {
            FileName = pgRestorePath,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
            CreateNoWindow = true
        };
        foreach (var arg in arguments)
        {
            startInfo.ArgumentList.Add(arg);
        }

        startInfo.Environment["PGPASSWORD"] = config.Password;

        if (!string.IsNullOrWhiteSpace(config.SslMode))
        {
            startInfo.Environment["PGSSLMODE"] = config.SslMode;
        }

        if (config.Timeout.HasValue)
        {
            startInfo.Environment["PGCONNECT_TIMEOUT"] = config.Timeout.Value.ToString();
        }

        if (config.CommandTimeout.HasValue)
        {
            startInfo.Environment["PGCOMMAND_TIMEOUT"] = config.CommandTimeout.Value.ToString();
        }

        try
        {
            using var process = Process.Start(startInfo);
            if (process == null)
            {
                throw new Exception("Failed to start pg_restore process.");
            }

            var output = process.StandardOutput.ReadToEnd();
            var error = process.StandardError.ReadToEnd();

            process.WaitForExit();

            if (process.ExitCode != 0)
            {
                throw new RestoreFailedException($"pg_restore failed with exit code {process.ExitCode}. Error: {error}");
            }

            if (!string.IsNullOrWhiteSpace(output))
            {
                _logger.LogDebug("{Output}", output);
            }
        }
        finally
        {
            startInfo.Environment["PGPASSWORD"] = string.Empty;
        }
    }

    /// <summary>
    /// Creates the target database if it does not already exist.
    /// </summary>
    /// <param name="host">The database host.</param>
    /// <param name="port">The database port.</param>
    /// <param name="username">The database username.</param>
    /// <param name="password">The database password.</param>
    /// <param name="databaseName">The name of the database to create if needed.</param>
    private void CreateDatabaseIfNeeded(string host, string port, string username, string password, string databaseName)
    {
        if (!SecurityUtils.IsValidPostgreSQLIdentifier(databaseName))
        {
            AnsiConsole.MarkupLine($"[red]Error:[/] Invalid database name: '{databaseName}'");
            throw new InvalidOperationException($"Invalid database name: '{databaseName}'");
        }

        var psqlPath = _binaryLocator.FindPsql();
        if (psqlPath == null)
        {
            throw new PostgresBinaryNotFoundException(FurLabConstants.PsqlExecutable);
        }

        var startInfo = new ProcessStartInfo
        {
            FileName = psqlPath,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
            CreateNoWindow = true
        };
        startInfo.ArgumentList.Add("-h");
        startInfo.ArgumentList.Add(host);
        startInfo.ArgumentList.Add("-p");
        startInfo.ArgumentList.Add(port);
        startInfo.ArgumentList.Add("-U");
        startInfo.ArgumentList.Add(username);
        startInfo.ArgumentList.Add("-d");
        startInfo.ArgumentList.Add("postgres");
        startInfo.ArgumentList.Add("-c");
        startInfo.ArgumentList.Add($"SELECT 1 FROM pg_database WHERE datname = '{databaseName}'");

        startInfo.Environment["PGPASSWORD"] = password;

        try
        {
            using var process = Process.Start(startInfo);
            if (process == null)
            {
                throw new Exception("Failed to start psql process.");
            }

            var output = process.StandardOutput.ReadToEnd();
            var error = process.StandardError.ReadToEnd();

            process.WaitForExit();

            if (process.ExitCode == 3)
            {
                var createStartInfo = new ProcessStartInfo
                {
                    FileName = psqlPath,
                    RedirectStandardOutput = true,
                    RedirectStandardError = true,
                    UseShellExecute = false,
                    CreateNoWindow = true
                };
                createStartInfo.ArgumentList.Add("-h");
                createStartInfo.ArgumentList.Add(host);
                createStartInfo.ArgumentList.Add("-p");
                createStartInfo.ArgumentList.Add(port);
                createStartInfo.ArgumentList.Add("-U");
                createStartInfo.ArgumentList.Add(username);
                createStartInfo.ArgumentList.Add("-d");
                createStartInfo.ArgumentList.Add("postgres");
                createStartInfo.ArgumentList.Add("-c");
                createStartInfo.ArgumentList.Add($"CREATE DATABASE \"{databaseName.Replace("\"", "\"\"")}\"");

                createStartInfo.Environment["PGPASSWORD"] = password;

                using var createProcess = Process.Start(createStartInfo);
                if (createProcess == null)
                {
                    throw new Exception("Failed to start psql process.");
                }

                createProcess.WaitForExit();

                if (createProcess.ExitCode != 0)
                {
                    var createError = createProcess.StandardError.ReadToEnd();
                    throw new Exception($"Failed to create database: {createError}");
                }
            }
        }
        finally
        {
            startInfo.Environment["PGPASSWORD"] = string.Empty;
        }
    }

    /// <summary>
    /// Builds the pg_restore command-line arguments from the configuration and settings.
    /// </summary>
    /// <param name="config">The validated restore configuration.</param>
    /// <param name="settings">The command settings.</param>
    /// <param name="filePath">The path to the dump file.</param>
    /// <returns>The formatted argument string for pg_restore.</returns>
    private List<string> BuildPgRestoreArguments(DatabaseRestoreConfig config, DatabaseRestoreSettings settings, string filePath)
    {
        var arguments = new List<string>
        {
            "-h", config.Host,
            "-p", config.Port,
            "-U", config.Username ?? string.Empty,
            "-d", config.DatabaseName
        };

        if (!settings.NoClean)
        {
            arguments.Add("-c");
        }

        if (settings.Verbose)
        {
            arguments.Add("-v");
        }

        if (settings.NoOwner)
        {
            arguments.Add("--no-owner");
        }

        if (settings.NoAcl)
        {
            arguments.Add("--no-acl");
        }

        if (settings.SingleTransaction)
        {
            arguments.Add("--single-transaction");
        }

        if (settings.Jobs.HasValue && settings.Jobs.Value > 0)
        {
            arguments.Add("-j");
            arguments.Add(settings.Jobs.Value.ToString());
        }

        arguments.Add(filePath);

        return arguments;
    }

    /// <summary>
    /// Prints a summary of the bulk restore operation results.
    /// </summary>
    /// <param name="successCount">The number of successfully restored databases.</param>
    /// <param name="failureCount">The number of failed restore operations.</param>
    /// <param name="totalCount">The total number of dump files processed.</param>
    /// <param name="inputDirectory">The directory that was scanned for dump files.</param>
    private void PrintRestoreSummary(int successCount, int failureCount, int totalCount, string inputDirectory)
    {
        _logger.LogInformation("========================================");
        _logger.LogInformation("Restore Summary:");
        _logger.LogInformation("  Successful: {Success}", successCount);
        _logger.LogInformation("  Failed: {Failure}", failureCount);
        _logger.LogInformation("  Total: {Total}", totalCount);
        _logger.LogInformation("  Input directory: {Directory}", Path.GetFullPath(inputDirectory));
        _logger.LogInformation("========================================");
    }
}
