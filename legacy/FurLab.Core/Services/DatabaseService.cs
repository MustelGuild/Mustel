using FurLab.Core.Interfaces;
using FurLab.Core.Models;

namespace FurLab.Core.Services;

/// <summary>
/// Provides methods for database listing and connection testing.
/// </summary>
/// <remarks>
/// Initializes a new instance of the <see cref="DatabaseService"/> class.
/// </remarks>
/// <param name="processExecutor">The process executor instance.</param>
/// <param name="postgresBinaryLocator">The PostgreSQL binary locator instance.</param>
public class DatabaseService(IProcessExecutor processExecutor, IPostgresBinaryLocator postgresBinaryLocator) : IDatabaseService
{
    private readonly IProcessExecutor _processExecutor = processExecutor ?? throw new ArgumentNullException(nameof(processExecutor));
    private readonly IPostgresBinaryLocator _postgresBinaryLocator = postgresBinaryLocator ?? throw new ArgumentNullException(nameof(postgresBinaryLocator));

    /// <summary>
    /// Lists all databases on the PostgreSQL server.
    /// </summary>
    /// <param name="options">The connection options.</param>
    /// <param name="cancellationToken">Optional cancellation token.</param>
    /// <returns>A task representing the operation with a list of database names.</returns>
    public async Task<List<string>> ListDatabasesAsync(
        DatabaseConnectionOptions options,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(options);

        var psqlPath = _postgresBinaryLocator.FindPsql() ?? throw new InvalidOperationException("psql executable not found. Please ensure PostgreSQL is installed.");
        var environmentVariables = new Dictionary<string, string>
        {
            ["PGPASSWORD"] = options.Password ?? string.Empty
        };

        var result = await _processExecutor.ExecuteAsync(
            new ProcessExecutionOptions
            {
                FileName = psqlPath,
                Arguments =
                [
                    "-h", options.Host ?? string.Empty,
                    "-p", options.Port ?? string.Empty,
                    "-U", options.Username ?? string.Empty,
                    "-d", "postgres",
                    "-c", "SELECT datname FROM pg_database WHERE datistemplate = false ORDER BY datname;"
                ],
                EnvironmentVariables = environmentVariables
            },
            cancellationToken: cancellationToken);

        if (!result.Success)
        {
            throw new InvalidOperationException($"Failed to list databases: {result.StandardError}");
        }

        // Parse output to get database names
        var databases = new List<string>();
        var lines = result.StandardOutput.Split('\n');

        foreach (var line in lines)
        {
            var trimmedLine = line.Trim();
            // Skip header lines and empty lines
            if (string.IsNullOrWhiteSpace(trimmedLine) ||
                trimmedLine.StartsWith("datname") ||
                trimmedLine.StartsWith("---") ||
                trimmedLine.StartsWith("("))
            {
                continue;
            }

            // Remove trailing | and whitespace
            if (trimmedLine.EndsWith("|"))
            {
                trimmedLine = trimmedLine[..^1].Trim();
            }

            if (!string.IsNullOrWhiteSpace(trimmedLine))
            {
                databases.Add(trimmedLine);
            }
        }

        return databases;
    }
}
