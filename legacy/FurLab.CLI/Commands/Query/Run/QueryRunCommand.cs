using System.Diagnostics;
using System.Globalization;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading.Channels;
using FurLab.Core.Constants;
using FurLab.Core.Interfaces;
using FurLab.Core.Models;
using Npgsql;
using Polly;
using Polly.Retry;
using Spectre.Console;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Query.Run;

/// <summary>
/// Executes a SQL query across one or more configured servers and exports results to CSV.
/// Supports inline queries and file-based input, server selection, destructive query confirmation,
/// Channels-based parallel execution, Polly retry, and progressive CSV output.
/// </summary>
public sealed class QueryRunCommand : AsyncCommand<QueryRunSettings>
{
    private readonly IUserConfigService _userConfigService;
    private readonly ICredentialService _credentialService;
    private readonly CsvExporter _csvExporter;

    private static readonly ResiliencePipeline ResiliencePipeline = new ResiliencePipelineBuilder()
        .AddRetry(new RetryStrategyOptions
        {
            MaxRetryAttempts = 3,
            DelayGenerator = static args =>
            {
                var delay = TimeSpan.FromMilliseconds(Math.Pow(2, args.AttemptNumber) * 500);
                return new ValueTask<TimeSpan?>(delay);
            },
            ShouldHandle = static args =>
            {
                var handled = args.Outcome.Exception is NpgsqlException or TimeoutException;
                return new ValueTask<bool>(handled);
            }
        })
        .Build();

    /// <summary>
    /// Initializes a new instance of the <see cref="QueryRunCommand"/> class.
    /// </summary>
    /// <param name="userConfigService">The user configuration service for server and defaults access.</param>
    /// <param name="credentialService">The credential service for password decryption.</param>
    /// <param name="csvExporter">The CSV exporter for writing query results.</param>
    public QueryRunCommand(IUserConfigService userConfigService, ICredentialService credentialService, CsvExporter csvExporter)
    {
        _userConfigService = userConfigService;
        _credentialService = credentialService;
        _csvExporter = csvExporter;
    }

    /// <inheritdoc/>
    protected override async Task<int> ExecuteAsync(CommandContext context, QueryRunSettings settings, CancellationToken cancellation)

    {
        if (!string.IsNullOrWhiteSpace(settings.Command) && !string.IsNullOrWhiteSpace(settings.Input))
        {
            AnsiConsole.MarkupLine("[red]Error:[/] Options -c/--command and -i/--input are mutually exclusive. Use only one.");
            return 1;
        }

        string sqlQuery;
        string querySource;

        if (!string.IsNullOrWhiteSpace(settings.Command))
        {
            sqlQuery = UnescapeInlineQuery(settings.Command);
            querySource = "inline query";
        }
        else if (!string.IsNullOrWhiteSpace(settings.Input))
        {
            if (!SecurityUtils.IsValidPath(settings.Input))
            {
                AnsiConsole.MarkupLine($"[red]Error:[/] Invalid input path: '{settings.Input.EscapeMarkup()}'. Path traversal not allowed.");
                return 1;
            }

            var inputFullPath = Path.GetFullPath(settings.Input);
            if (!File.Exists(inputFullPath))
            {
                AnsiConsole.MarkupLine($"[red]Error:[/] SQL input file not found: {inputFullPath.EscapeMarkup()}");
                return 1;
            }

            sqlQuery = File.ReadAllText(inputFullPath, Encoding.UTF8);
            querySource = inputFullPath;
        }
        else
        {
            AnsiConsole.MarkupLine("[red]Error:[/] Use -c for an inline query or -i for a SQL file.");
            return 1;
        }

        if (string.IsNullOrWhiteSpace(sqlQuery))
        {
            AnsiConsole.MarkupLine("[red]Error:[/] SQL query is empty.");
            return 1;
        }

        var queryType = SqlQueryAnalyzer.AnalyzeQuery(sqlQuery);
        var queryTypeDescription = SqlQueryAnalyzer.GetQueryTypeDescription(sqlQuery);

        List<ServerConfigEntry> selectedServers;

        if (!string.IsNullOrWhiteSpace(settings.NpgsqlConnectionString))
        {
            var adHocServer = CreateAdHocServer(settings);
            selectedServers = [adHocServer];
        }
        else
        {
            var servers = _userConfigService.GetServers();
            if (servers.Count == 0)
            {
                AnsiConsole.MarkupLine("[red]Error:[/] No servers configured. Run 'settings db-servers add' to add a server first.");
                return 1;
            }

            selectedServers = SelectServers(servers);
            if (selectedServers.Count == 0)
            {
                AnsiConsole.MarkupLine("[yellow]No servers selected. Execution cancelled.[/]");
                return 1;
            }

            if (settings.All)
            {
                foreach (var server in selectedServers)
                {
                    server.FetchAllDatabases = true;
                }
            }
        }

        var excludeNames = ParseExcludeList(settings.Exclude);

        if (queryType == QueryType.Destructive && !settings.NoConfirm)
        {
            var defaults = _userConfigService.GetDefaults();
            if (defaults.RequireConfirmation)
            {
                var databaseCount = selectedServers.Sum(s => s.FetchAllDatabases ? 1 : Math.Max(s.Databases.Count, 1));
                if (!ConfirmDestructiveQuery(queryTypeDescription, selectedServers, databaseCount, sqlQuery))
                {
                    AnsiConsole.MarkupLine("[yellow]Query execution cancelled by user.[/]");
                    return 1;
                }
            }
        }

        try
        {
            await ExecuteOnSelectedServers(selectedServers, sqlQuery, settings, querySource, queryTypeDescription, excludeNames, cancellation);
            return 0;
        }
        catch (InvalidOperationException ex)
        {
            AnsiConsole.MarkupLine($"[red]Error:[/] {ex.Message.EscapeMarkup()}");
            return 1;
        }
    }

    /// <summary>
    /// Creates an ad-hoc <see cref="ServerConfigEntry"/> from the connection string settings.
    /// Used when <c>--npgsql-connection-string</c> is provided, bypassing server selection.
    /// Individual connection options (<c>--host</c>, <c>--port</c>, etc.) override the parsed connection string values.
    /// </summary>
    private static ServerConfigEntry CreateAdHocServer(QueryRunSettings settings)
    {
        var builder = new NpgsqlConnectionStringBuilder(settings.NpgsqlConnectionString);

        if (!string.IsNullOrWhiteSpace(settings.Host))
        {
            builder.Host = settings.Host;
        }

        if (!string.IsNullOrWhiteSpace(settings.Port))
        {
            builder.Port = int.TryParse(settings.Port, out var port) ? port : builder.Port;
        }

        if (!string.IsNullOrWhiteSpace(settings.Database))
        {
            builder.Database = settings.Database;
        }

        if (!string.IsNullOrWhiteSpace(settings.Username))
        {
            builder.Username = settings.Username;
        }

        if (!string.IsNullOrWhiteSpace(settings.Password))
        {
            builder.Password = settings.Password;
        }

        if (!string.IsNullOrWhiteSpace(settings.SslMode))
        {
            builder.SslMode = ParseSslMode(settings.SslMode);
        }

        if (settings.Timeout.HasValue)
        {
            builder.Timeout = settings.Timeout.Value;
        }

        if (settings.CommandTimeout.HasValue)
        {
            builder.CommandTimeout = settings.CommandTimeout.Value;
        }

        if (settings.Pooling.HasValue)
        {
            builder.Pooling = settings.Pooling.Value;
        }

        if (settings.MinPoolSize.HasValue)
        {
            builder.MinPoolSize = settings.MinPoolSize.Value;
        }

        if (settings.MaxPoolSize.HasValue)
        {
            builder.MaxPoolSize = settings.MaxPoolSize.Value;
        }

        if (settings.Keepalive.HasValue)
        {
            builder["keepalive"] = settings.Keepalive.Value;
        }

        if (settings.ConnectionLifetime.HasValue)
        {
            builder["connection lifetime"] = settings.ConnectionLifetime.Value;
        }

        var database = builder.Database ?? "postgres";

        return new ServerConfigEntry
        {
            Name = builder.Host ?? "ad-hoc",
            Host = builder.Host ?? FurLabConstants.DefaultHost,
            Port = builder.Port,
            Username = builder.Username ?? string.Empty,
            EncryptedPassword = null,
            Databases = [database],
            FetchAllDatabases = settings.All,
            SslMode = builder.SslMode.ToString(),
            Timeout = builder.Timeout,
            MaxParallelism = 4
        };
    }

    /// <summary>
    /// Parses a comma-separated exclude list into a set of database names (trimmed, empty entries removed).
    /// </summary>
    private static HashSet<string> ParseExcludeList(string? exclude)
    {
        if (string.IsNullOrWhiteSpace(exclude))
        {
            return [];
        }

        return new HashSet<string>(
            exclude.Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries),
            StringComparer.OrdinalIgnoreCase);
    }

    /// <summary>
    /// Unescapes an inline SQL query string received from the command line.
    /// </summary>
    /// <remarks>
    /// <para>
    /// On Windows, the shell and <c>System.CommandLine</c> do not always strip
    /// the outer quotes from arguments, especially when the value contains spaces
    /// or special characters. This method performs a best-effort strip of a single
    /// outer quote pair (either <c>"…"</c> or <c>'…'</c>) and then unescapes
    /// any backslash-escaped quotes inside the value.
    /// </para>
    /// <para>
    /// Examples:
    /// <list type="bullet">
    ///   <item><description><c>"SELECT 1"</c> → <c>SELECT 1</c></description></item>
    ///   <item><description><c>'SELECT 1'</c> → <c>SELECT 1</c></description></item>
    ///   <item><description><c>SELECT \"name\" FROM t</c> → <c>SELECT "name" FROM t</c></description></item>
    ///   <item><description><c>SELECT 1</c> (no outer quotes) → <c>SELECT 1</c> (unchanged)</description></item>
    /// </list>
    /// </para>
    /// </remarks>
    internal static string UnescapeInlineQuery(string query)
    {
        if ((query.StartsWith('"') && query.EndsWith('"')) ||
            (query.StartsWith('\'') && query.EndsWith('\'')))
        {
            query = query.Substring(1, query.Length - 2);
        }

        query = query.Replace("\\\"", "\"").Replace("\\'", "'");
        return query;
    }

    /// <summary>
    /// Shows interactive server selection prompt with all servers pre-selected.
    /// If only one server is configured, returns it directly without a prompt.
    /// </summary>
    private static List<ServerConfigEntry> SelectServers(IReadOnlyList<ServerConfigEntry> servers)
    {
        if (servers.Count == 1)
        {
            return [servers[0]];
        }

        var prompt = new MultiSelectionPrompt<string>()
            .Title("Select servers to execute query on:")
            .PageSize(10)
            .MoreChoicesText("[grey](Move up and down to reveal more servers)[/]")
            .InstructionsText("[grey](Press <space> to toggle, <enter> to accept)[/]")
            .AddChoices(servers.Select(s => s.Name));

        foreach (var server in servers)
        {
            prompt.Select(server.Name);
        }

        var selected = AnsiConsole.Prompt(prompt);
        return servers.Where(s => selected.Contains(s.Name)).ToList();
    }

    /// <summary>
    /// Shows a confirmation prompt for destructive queries, displaying the query type,
    /// number of affected servers and databases, and a preview of the SQL.
    /// </summary>
    private static bool ConfirmDestructiveQuery(string queryType, List<ServerConfigEntry> selectedServers, int databaseCount, string sqlQuery)
    {
        var preview = sqlQuery.Length > 200 ? sqlQuery.Substring(0, 200) + "..." : sqlQuery;

        var table = new Table();
        table.AddColumn("Property");
        table.AddColumn("Value");
        table.AddRow("Query Type", $"[red]{queryType}[/]");
        table.AddRow("Servers Affected", $"[yellow]{selectedServers.Count}[/]");
        table.AddRow("Databases Affected", $"[yellow]~{databaseCount}[/]");
        table.AddRow("Preview", $"[grey]{Markup.Escape(preview)}[/]");

        AnsiConsole.WriteLine();
        AnsiConsole.MarkupLine("[bold red]WARNING: DESTRUCTIVE QUERY DETECTED[/]");
        AnsiConsole.Write(table);
        AnsiConsole.WriteLine();

        return AnsiConsole.Confirm("Proceed with execution?");
    }

    /// <summary>
    /// Executes the query on all selected servers with parallel execution, progressive CSV output,
    /// and a Spectre.Console progress bar with live activity feed.
    /// Writes per-server partial CSVs progressively via a Channel-based single writer task,
    /// then merges them into a consolidated CSV at the end. Errors and all executions are
    /// also logged progressively to dedicated CSV files.
    /// </summary>
    private async Task ExecuteOnSelectedServers(
        List<ServerConfigEntry> selectedServers,
        string sqlQuery,
        QueryRunSettings settings,
        string querySource,
        string queryTypeDescription,
        HashSet<string> excludeNames,
        CancellationToken cancellationToken)
    {
        var defaults = _userConfigService.GetDefaults();
        var baseOutputDirectory = string.IsNullOrWhiteSpace(settings.Output)
            ? defaults.OutputDirectory
            : Path.GetFullPath(settings.Output);

        var timestamp = DateTime.UtcNow.ToString("yyyy-MM-dd_HHmmss", CultureInfo.InvariantCulture);
        var executionDirectory = Path.Combine(baseOutputDirectory, timestamp);

        if (!Directory.Exists(executionDirectory))
        {
            Directory.CreateDirectory(executionDirectory);
        }

        var errorFilePath = Path.Combine(executionDirectory, $"{timestamp}_erros.csv");
        var logFilePath = Path.Combine(executionDirectory, $"{timestamp}_log.csv");

        using var cts = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        Console.CancelKeyPress += (_, e) =>
        {
            e.Cancel = true;
            cts.Cancel();
        };

        var allDatabases = new List<(ServerConfigEntry Server, string Database)>();
        var hasAutoDiscover = selectedServers.Any(s => s.FetchAllDatabases);

        if (hasAutoDiscover)
        {
            await AnsiConsole.Status()
                .Spinner(Spinner.Known.Dots)
                .SpinnerStyle(Style.Parse("grey"))
                .StartAsync("Discovering databases...", async ctx =>
                {
                    foreach (var server in selectedServers)
                    {
                        ctx.Status($"Discovering databases on [bold]{Markup.Escape(server.Name)}[/]...");
                        var databases = await GetDatabasesForServerAsync(server, excludeNames, cts.Token);
                        foreach (var db in databases)
                        {
                            allDatabases.Add((server, db));
                        }
                    }
                });
        }
        else
        {
            foreach (var server in selectedServers)
            {
                var databases = await GetDatabasesForServerAsync(server, excludeNames, cts.Token);
                foreach (var db in databases)
                {
                    allDatabases.Add((server, db));
                }
            }
        }

        var totalDatabases = allDatabases.Count;

        if (totalDatabases == 0)
        {
            AnsiConsole.MarkupLine("[yellow]No accessible databases found across selected servers.[/]");
            return;
        }

        var successCount = 0;
        var failureCount = 0;
        var totalRowCount = 0;

        var channel = Channel.CreateBounded<CsvRow>(
            new BoundedChannelOptions(defaults.MaxParallelism * 2)
            {
                FullMode = BoundedChannelFullMode.Wait,
                SingleReader = true
            });

        var writerCompleted = new TaskCompletionSource();

        var csvExporter = _csvExporter;

        _ = Task.Run(async () =>
        {
            try
            {
                await foreach (var row in channel.Reader.ReadAllAsync())
                {
                    if (row.Status == "Success")
                    {
                        var serverFileName = csvExporter.SanitizeFilename(row.Server);
                        var serverCsvPath = Path.Combine(executionDirectory, $"{serverFileName}_{timestamp}.csv");
                        csvExporter.AppendToServerCsv(serverCsvPath, row);
                    }
                    else
                    {
                        csvExporter.WriteErrorEntry(errorFilePath, row.Server, row.Database, row.ExecutedAt, row.Error);
                    }

                    var logEntry = new ExecutionLogEntry(row.Server, row.Database, row.ExecutedAt, row.Status, row.RowCount, row.DurationMs, row.Error);
                    csvExporter.WriteLogEntry(logFilePath, logEntry);
                }
            }
            catch (Exception ex)
            {
                writerCompleted.TrySetException(ex);
                return;
            }

            writerCompleted.TrySetResult();
        });

        var resultsTable = new Table()
            .Border(TableBorder.Rounded)
            .AddColumn(new TableColumn("[grey]Status[/]").Centered().Width(8))
            .AddColumn(new TableColumn("[grey]Server[/]"))
            .AddColumn(new TableColumn("[grey]Database[/]"))
            .AddColumn(new TableColumn("[grey]Rows[/]").RightAligned())
            .AddColumn(new TableColumn("[grey]Duration[/]").RightAligned())
            .AddColumn(new TableColumn("[grey]Detail[/]"));

        var tableLock = new object();

        await AnsiConsole.Live(resultsTable)
            .AutoClear(false)
            .Overflow(VerticalOverflow.Ellipsis)
            .StartAsync(async ctx =>
            {
                ctx.Refresh();

                var parallelOptions = new ParallelOptions
                {
                    MaxDegreeOfParallelism = defaults.MaxParallelism,
                    CancellationToken = cts.Token
                };

                await Parallel.ForEachAsync(selectedServers, parallelOptions, async (server, ct) =>
                {
                    var databases = allDatabases
                        .Where(d => d.Server.Name == server.Name)
                        .Select(d => d.Database)
                        .ToList();

                    if (databases.Count == 0)
                    {
                        lock (tableLock)
                        {
                            resultsTable.AddRow("[yellow]—[/]", Markup.Escape(server.Name), "[grey]—[/]", "[grey]—[/]", "[grey]—[/]", "[yellow]no databases[/]");
                            ctx.Refresh();
                        }
                        return;
                    }

                    var serverParallelOptions = new ParallelOptions
                    {
                        MaxDegreeOfParallelism = server.MaxParallelism,
                        CancellationToken = ct
                    };

                    await Parallel.ForEachAsync(databases, serverParallelOptions, async (database, dbCt) =>
                    {
                        try
                        {
                            var connectionString = BuildConnectionStringForServer(server, database, settings);
                            var executedAt = DateTime.UtcNow;
                            var sw = Stopwatch.StartNew();
                            var queryResult = await ExecuteQueryWithRetryAsync(connectionString, sqlQuery, settings?.CommandTimeout ?? 300, dbCt);
                            sw.Stop();

                            var row = new CsvRow(server.Name, database, executedAt, "Success", queryResult.Data.Count, string.Empty, sw.Elapsed.TotalMilliseconds, queryResult.ColumnNames, queryResult.Data);
                            await channel.Writer.WriteAsync(row, dbCt);

                            Interlocked.Increment(ref successCount);
                            Interlocked.Add(ref totalRowCount, queryResult.Data.Count);

                            lock (tableLock)
                            {
                                resultsTable.AddRow(
                                    "[green]✓[/]",
                                    Markup.Escape(server.Name),
                                    Markup.Escape(database),
                                    queryResult.Data.Count.ToString(CultureInfo.InvariantCulture),
                                    $"{sw.Elapsed.TotalSeconds:F1}s",
                                    "[grey]—[/]");
                                ctx.Refresh();
                            }
                        }
                        catch (Exception ex)
                        {
                            var executedAt = DateTime.UtcNow;
                            var row = new CsvRow(server.Name, database, executedAt, "Error", 0, ex.Message, 0, [], []);
                            await channel.Writer.WriteAsync(row, dbCt);

                            Interlocked.Increment(ref failureCount);

                            lock (tableLock)
                            {
                                resultsTable.AddRow(
                                    "[red]✗[/]",
                                    Markup.Escape(server.Name),
                                    Markup.Escape(database),
                                    "[grey]—[/]",
                                    "[grey]—[/]",
                                    $"[red]{Markup.Escape(ex.Message)}[/]");
                                ctx.Refresh();
                            }
                        }
                    });
                });
            });

        channel.Writer.Complete();
        await writerCompleted.Task.WaitAsync(cts.Token);

        _csvExporter.MergeServerCsvsToConsolidated(executionDirectory, timestamp, selectedServers.Select(s => s.Name).ToList());

        var consolidatedPath = Path.Combine(executionDirectory, $"consolidated_{timestamp}.csv");
        AnsiConsole.WriteLine();

        if (File.Exists(consolidatedPath))
        {
            AnsiConsole.MarkupLine($"[green]✅ Consolidated →[/] {Markup.Escape(consolidatedPath)}");
        }

        if (File.Exists(errorFilePath))
        {
            AnsiConsole.MarkupLine($"[red]❌ Errors       →[/] {Markup.Escape(errorFilePath)}");
        }

        AnsiConsole.MarkupLine($"[grey]Log           →[/] {Markup.Escape(logFilePath)}");
        AnsiConsole.WriteLine();
        AnsiConsole.MarkupLine($"Servers: [bold]{selectedServers.Count}[/] | [green]Success: {successCount}[/] | [red]Failed: {failureCount}[/] | Total rows: {totalRowCount}");

        if (successCount == 0 && failureCount > 0)
        {
            throw new InvalidOperationException("No server responded successfully. Please check the connections.");
        }
    }

    /// <summary>
    /// Executes a query with Polly retry logic for transient failures (up to 3 retries with exponential backoff).
    /// </summary>
    private static async Task<(List<string> ColumnNames, List<Dictionary<string, string>> Data)> ExecuteQueryWithRetryAsync(string connectionString, string sqlQuery, int commandTimeout, CancellationToken ct)
    {
        return await ResiliencePipeline.ExecuteAsync(async (innerCt) =>
        {
            innerCt.ThrowIfCancellationRequested();
            return await ExecuteQueryAsync(connectionString, sqlQuery, commandTimeout, innerCt);
        }, ct);
    }

    /// <summary>
    /// Executes a query and returns column names and all data rows as string dictionaries.
    /// </summary>
    private static async Task<(List<string> ColumnNames, List<Dictionary<string, string>> Data)> ExecuteQueryAsync(string connectionString, string sqlQuery, int commandTimeout, CancellationToken ct)
    {
        await using var connection = new NpgsqlConnection(connectionString);
        await connection.OpenAsync(ct);

        await using var command = new NpgsqlCommand(sqlQuery, connection)
        {
            CommandTimeout = commandTimeout
        };

        await using var reader = await command.ExecuteReaderAsync(ct);

        var columnNames = new List<string>();
        for (var i = 0; i < reader.FieldCount; i++)
        {
            columnNames.Add(reader.GetName(i));
        }

        var data = new List<Dictionary<string, string>>();
        while (await reader.ReadAsync(ct))
        {
            var row = new Dictionary<string, string>();
            for (var i = 0; i < reader.FieldCount; i++)
            {
                var value = reader.IsDBNull(i) ? string.Empty : reader.GetValue(i)?.ToString() ?? string.Empty;
                row[columnNames[i]] = value;
            }
            data.Add(row);
        }

        return (columnNames, data);
    }

    /// <summary>
    /// Gets the list of databases for a server.
    /// When <c>FetchAllDatabases</c> is true, auto-discovers via <c>pg_database</c> query,
    /// falling back to configured databases if discovery fails.
    /// Otherwise, returns the databases listed in the server configuration directly.
    /// Databases matching entries in <paramref name="excludeNames"/> are removed from the result.
    /// </summary>
    private async Task<List<string>> GetDatabasesForServerAsync(ServerConfigEntry server, HashSet<string> excludeNames, CancellationToken ct)
    {
        List<string> databases;

        if (server.FetchAllDatabases)
        {
            try
            {
                databases = await ListDatabasesAsync(server, ct);
            }
            catch (Exception ex)
            {
                AnsiConsole.MarkupLine($"[yellow]Warning: Auto-discovery failed for '{server.Name.EscapeMarkup()}': {ex.Message.EscapeMarkup()}[/]");
                if (server.Databases.Count > 0)
                {
                    AnsiConsole.MarkupLine("[yellow]Falling back to configured databases.[/]");
                    databases = server.Databases;
                }
                else
                {
                    return [];
                }
            }
        }
        else
        {
            if (server.Databases.Count > 0)
            {
                databases = server.Databases;
            }
            else
            {
                return [];
            }
        }

        if (excludeNames.Count > 0)
        {
            databases = databases.Where(db => !excludeNames.Contains(db)).ToList();
        }

        return databases;
    }

    /// <summary>
    /// Lists all databases on a server using <c>pg_database</c>, filtered by the server's exclude patterns.
    /// </summary>
    private async Task<List<string>> ListDatabasesAsync(ServerConfigEntry server, CancellationToken ct)
    {
        var connectionString = BuildConnectionStringForServer(server, "postgres", null);
        var databases = new List<string>();

        await using var connection = new NpgsqlConnection(connectionString);
        await connection.OpenAsync(ct);

        await using var command = new NpgsqlCommand(
            "SELECT datname FROM pg_database WHERE datistemplate = false AND datallowconn = true",
            connection);

        await using var reader = await command.ExecuteReaderAsync(ct);
        while (await reader.ReadAsync(ct))
        {
            var dbName = reader.GetString(0);
            if (!server.ExcludePatterns.Any(pattern => MatchesPattern(dbName, pattern)))
            {
                databases.Add(dbName);
            }
        }

        return databases;
    }

    /// <summary>
    /// Checks if a database name matches a wildcard pattern (supports <c>*</c> as a multi-character wildcard).
    /// Matching is case-insensitive.
    /// </summary>
    private static bool MatchesPattern(string dbName, string pattern)
    {
        var regexPattern = "^" + System.Text.RegularExpressions.Regex.Escape(pattern).Replace("\\*", ".*") + "$";
        return System.Text.RegularExpressions.Regex.IsMatch(dbName, regexPattern, System.Text.RegularExpressions.RegexOptions.IgnoreCase);
    }

    /// <summary>
    /// Builds a connection string for a specific server and database, applying any connection
    /// setting overrides from the command line.
    /// Resolves the password via <c>ICredentialService.TryDecrypt</c>; if unavailable, prompts interactively.
    /// For ad-hoc connections created from <c>--npgsql-connection-string</c>, the password is embedded directly
    /// into the connection string builder and the interactive fallback is skipped.
    /// </summary>
    private string BuildConnectionStringForServer(ServerConfigEntry server, string database, QueryRunSettings? settings)
    {
        var password = ResolvePassword(server, settings);

        var builder = new NpgsqlConnectionStringBuilder
        {
            Host = ResolveSetting(settings?.Host, server.Host),
            Port = ResolvePort(settings?.Port, server.Port),
            Database = database,
            Username = ResolveSetting(settings?.Username, server.Username),
            Password = password,
            SslMode = ParseSslMode(ResolveSetting(settings?.SslMode, server.SslMode)),
            Timeout = settings?.Timeout ?? server.Timeout,
            Pooling = settings?.Pooling ?? true,
            MinPoolSize = settings?.MinPoolSize ?? 1,
            MaxPoolSize = settings?.MaxPoolSize ?? 100
        };

        if (settings?.Keepalive.HasValue == true)
        {
            builder["keepalive"] = settings.Keepalive.Value;
        }

        if (settings?.ConnectionLifetime.HasValue == true)
        {
            builder["connection lifetime"] = settings.ConnectionLifetime.Value;
        }

        return builder.ConnectionString;
    }

    /// <summary>
    /// Resolves the password for a server, using the <c>--password</c> setting override if provided,
    /// then <c>ICredentialService.TryDecrypt</c>, and finally falling back to interactive input.
    /// </summary>
    private string ResolvePassword(ServerConfigEntry server, QueryRunSettings? settings = null)
    {
        if (!string.IsNullOrWhiteSpace(settings?.Password))
        {
            return settings.Password;
        }

        var decrypted = _credentialService.TryDecrypt(server.EncryptedPassword);
        if (decrypted != null)
        {
            return decrypted;
        }

        AnsiConsole.MarkupLine($"[yellow]No password found for server '[bold]{Markup.Escape(server.Name)}[/]'. Use 'fur settings db-servers set-password' to save it permanently.[/]");
        return ReadPasswordInteractive(server.Name);
    }

    /// <summary>
    /// Resolves a setting value, preferring the CLI override when provided, falling back to the server config value.
    /// </summary>
    private static string? ResolveSetting(string? overrideValue, string configValue)
    {
        return string.IsNullOrWhiteSpace(overrideValue) ? configValue : overrideValue;
    }

    /// <summary>
    /// Resolves a port number, preferring the CLI override when provided, falling back to the server config value.
    /// </summary>
    private static int ResolvePort(string? overrideValue, int configValue)
    {
        return int.TryParse(overrideValue, out var port) ? port : configValue;
    }

    /// <summary>
    /// Parses an SSL mode string into an <see cref="SslMode"/> enum value.
    /// Returns <see cref="SslMode.Prefer"/> if the value cannot be parsed.
    /// </summary>
    private static SslMode ParseSslMode(string? sslMode)
    {
        if (!string.IsNullOrWhiteSpace(sslMode) && Enum.TryParse<SslMode>(sslMode, true, out var result))
        {
            return result;
        }

        return SslMode.Prefer;
    }

    /// <summary>
    /// Reads a password interactively from the console with masked input.
    /// Used as fallback when the encrypted password is unavailable and no CLI override was provided.
    /// </summary>
    private static string ReadPasswordInteractive(string serverName)
    {
        AnsiConsole.Markup($"[dim]Password for '{Markup.Escape(serverName)}': [/]");
        var securePassword = new System.Security.SecureString();
        while (true)
        {
            var key = Console.ReadKey(true);
            if (key.Key == ConsoleKey.Enter)
            {
                Console.WriteLine();
                break;
            }

            if (key.Key == ConsoleKey.Backspace && securePassword.Length > 0)
            {
                securePassword.RemoveAt(securePassword.Length - 1);
                Console.Write("\b \b");
            }
            else if (key.Key != ConsoleKey.Backspace)
            {
                securePassword.AppendChar(key.KeyChar);
                Console.Write("*");
            }
        }

        var ptr = Marshal.SecureStringToBSTR(securePassword);
        try
        {
            return Marshal.PtrToStringBSTR(ptr) ?? string.Empty;
        }
        finally
        {
            Marshal.ZeroFreeBSTR(ptr);
        }
    }
}
