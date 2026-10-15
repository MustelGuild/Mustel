using System.Globalization;
using System.Text;

namespace FurLab.CLI.Commands.Query;

/// <summary>
/// Provides CSV export functionality for query results.
/// Supports progressive append per server and consolidated merge at the end of execution.
/// </summary>
public class CsvExporter
{
    /// <summary>
    /// Writes a consolidated CSV file by merging all success results from multiple servers and databases.
    /// Columns: Server, Database, then all query result columns (union of all result sets, in first-appearance order).
    /// </summary>
    /// <param name="outputPath">Destination file path.</param>
    /// <param name="successResults">Query results with Status == "Success".</param>
    public void WriteConsolidatedCsv(string outputPath, List<CsvRow> successResults)
    {
        using var writer = new StreamWriter(outputPath, false, Encoding.UTF8);
        using var csv = new CsvHelper.CsvWriter(writer, CultureInfo.InvariantCulture);
        WriteConsolidatedCsv(csv, successResults);
    }

    /// <summary>
    /// Writes consolidated CSV rows to an already-open CsvWriter.
    /// Used for testing without requiring file I/O.
    /// </summary>
    internal void WriteConsolidatedCsv(CsvHelper.CsvWriter csv, List<CsvRow> successResults)
    {
        var allColumnNames = BuildColumnList(successResults);

        csv.WriteField("Server");
        csv.WriteField("Database");
        foreach (var columnName in allColumnNames)
        {
            csv.WriteField(columnName);
        }
        csv.NextRecord();

        foreach (var result in successResults)
        {
            foreach (var dataRow in result.Data)
            {
                csv.WriteField(result.Server);
                csv.WriteField(result.Database);
                foreach (var columnName in allColumnNames)
                {
                    var value = dataRow.ContainsKey(columnName) ? dataRow[columnName] : string.Empty;
                    csv.WriteField(value);
                }
                csv.NextRecord();
            }
        }
    }

    /// <summary>
    /// Appends a query result row to the per-server CSV file.
    /// Creates the file with a header (Server, Database, &lt;query columns&gt;) on first write;
    /// subsequent calls append data rows only. Uses <c>AutoFlush = true</c> to flush each write to disk immediately.
    /// </summary>
    /// <remarks>
    /// If databases on the same server return different columns, the header written on first call
    /// will be inconsistent with later data rows. This is acceptable — the consolidated CSV generated
    /// at the end of execution resolves inconsistencies with a unified header.
    /// </remarks>
    /// <param name="outputPath">Destination file path (created if it does not exist).</param>
    /// <param name="row">The query result row to append.</param>
    public void AppendToServerCsv(string outputPath, CsvRow row)
    {
        var fileExists = File.Exists(outputPath);
        using var writer = new StreamWriter(outputPath, append: true, Encoding.UTF8)
        {
            AutoFlush = true
        };
        using var csv = new CsvHelper.CsvWriter(writer, CultureInfo.InvariantCulture);

        if (!fileExists)
        {
            csv.WriteField("Server");
            csv.WriteField("Database");
            foreach (var columnName in row.ColumnNames)
            {
                csv.WriteField(columnName);
            }
            csv.NextRecord();
        }

        foreach (var dataRow in row.Data)
        {
            csv.WriteField(row.Server);
            csv.WriteField(row.Database);
            foreach (var columnName in row.ColumnNames)
            {
                var value = dataRow.ContainsKey(columnName) ? dataRow[columnName] : string.Empty;
                csv.WriteField(value);
            }
            csv.NextRecord();
        }
    }

    /// <summary>
    /// Appends a query failure entry to the errors CSV file.
    /// Creates the file with a header (Server, Database, ExecutedAt, Error) on first write.
    /// Uses <c>AutoFlush = true</c> to flush each write to disk immediately.
    /// </summary>
    /// <param name="outputPath">Destination file path (created if it does not exist).</param>
    /// <param name="server">Name of the server where the error occurred.</param>
    /// <param name="database">Name of the database where the error occurred.</param>
    /// <param name="executedAt">Timestamp of the failed execution attempt.</param>
    /// <param name="error">Error message.</param>
    public void WriteErrorEntry(string outputPath, string server, string database, DateTime executedAt, string error)
    {
        var fileExists = File.Exists(outputPath);
        using var writer = new StreamWriter(outputPath, append: true, Encoding.UTF8)
        {
            AutoFlush = true
        };
        using var csv = new CsvHelper.CsvWriter(writer, CultureInfo.InvariantCulture);

        if (!fileExists)
        {
            csv.WriteField("Server");
            csv.WriteField("Database");
            csv.WriteField("ExecutedAt");
            csv.WriteField("Error");
            csv.NextRecord();
        }

        csv.WriteField(server);
        csv.WriteField(database);
        csv.WriteField(executedAt.ToString("O", CultureInfo.InvariantCulture));
        csv.WriteField(error);
        csv.NextRecord();
    }

    /// <summary>
    /// Appends an execution log entry to the execution log CSV file.
    /// Creates the file with a header (Server, Database, ExecutedAt, Status, RowCount, DurationMs, Error) on first write.
    /// Written for every query result — both successes and failures. Uses <c>AutoFlush = true</c> to flush immediately.
    /// </summary>
    /// <param name="outputPath">Destination file path (created if it does not exist).</param>
    /// <param name="entry">The execution log entry to append.</param>
    public void WriteLogEntry(string outputPath, ExecutionLogEntry entry)
    {
        var fileExists = File.Exists(outputPath);
        using var writer = new StreamWriter(outputPath, append: true, Encoding.UTF8)
        {
            AutoFlush = true
        };
        using var csv = new CsvHelper.CsvWriter(writer, CultureInfo.InvariantCulture);

        if (!fileExists)
        {
            csv.WriteField("Server");
            csv.WriteField("Database");
            csv.WriteField("ExecutedAt");
            csv.WriteField("Status");
            csv.WriteField("RowCount");
            csv.WriteField("DurationMs");
            csv.WriteField("Error");
            csv.NextRecord();
        }

        csv.WriteField(entry.Server);
        csv.WriteField(entry.Database);
        csv.WriteField(entry.ExecutedAt.ToString("O", CultureInfo.InvariantCulture));
        csv.WriteField(entry.Status);
        csv.WriteField(entry.RowCount);
        csv.WriteField(entry.DurationMs);
        csv.WriteField(entry.Error);
        csv.NextRecord();
    }

    /// <summary>
    /// Builds an ordered, deduplicated list of column names from all result sets.
    /// Column order is determined by first appearance across results.
    /// </summary>
    internal List<string> BuildColumnList(List<CsvRow> results)
    {
        var allColumnNames = new List<string>();
        var seenColumns = new HashSet<string>();
        foreach (var result in results)
        {
            foreach (var columnName in result.ColumnNames)
            {
                if (seenColumns.Add(columnName))
                {
                    allColumnNames.Add(columnName);
                }
            }
        }
        return allColumnNames;
    }

    private static readonly char[] WindowsInvalidFileNameChars =
        ['/', '\\', ':', '*', '?', '"', '<', '>', '|', '\0'];

    /// <summary>
    /// Sanitizes a server name for use as a filename by replacing all invalid filename characters with <c>_</c>.
    /// </summary>
    /// <param name="name">The raw server name.</param>
    /// <returns>A string safe for use as a filename component.</returns>
    internal string SanitizeFilename(string name)
    {
        var sb = new StringBuilder(name.Length);
        foreach (var c in name)
        {
            sb.Append(WindowsInvalidFileNameChars.Contains(c) ? '_' : c);
        }
        return sb.ToString();
    }

    /// <summary>
    /// Merges per-server partial CSV files into a single consolidated CSV with a unified header.
    /// Reads each server's partial file, reconstructs rows, then calls <see cref="WriteConsolidatedCsv(string, List{CsvRow})"/>
    /// to produce <c>consolidated_&lt;timestamp&gt;.csv</c> in the output directory.
    /// Does nothing if no partial files exist or if no rows were found.
    /// </summary>
    /// <param name="outputDirectory">Directory that contains the per-server partial CSV files.</param>
    /// <param name="timestamp">Timestamp string used to locate partial files and name the consolidated output.</param>
    /// <param name="serverNames">Names of the servers whose partial files should be merged (unsanitized; sanitization is applied internally).</param>
    public void MergeServerCsvsToConsolidated(string outputDirectory, string timestamp, List<string> serverNames)
    {
        var allResults = new List<CsvRow>();
        foreach (var serverName in serverNames)
        {
            var serverFile = Path.Combine(outputDirectory, $"{SanitizeFilename(serverName)}_{timestamp}.csv");
            if (!File.Exists(serverFile)) continue;

            var rows = ReadServerCsv(serverFile);
            allResults.AddRange(rows);
        }

        if (allResults.Count == 0) return;

        var consolidatedPath = Path.Combine(outputDirectory, $"consolidated_{timestamp}.csv");
        WriteConsolidatedCsv(consolidatedPath, allResults);
    }

    /// <summary>
    /// Reads a per-server partial CSV file and reconstructs a list of <see cref="CsvRow"/> objects.
    /// Used internally by <see cref="MergeServerCsvsToConsolidated"/> to prepare rows for the consolidated merge.
    /// </summary>
    private List<CsvRow> ReadServerCsv(string filePath)
    {
        using var reader = new StreamReader(filePath, Encoding.UTF8);
        using var csv = new CsvHelper.CsvReader(reader, CultureInfo.InvariantCulture);

        var records = csv.GetRecords<dynamic>().ToList();
        if (records.Count == 0) return [];

        var header = csv.HeaderRecord;
        if (header == null) return [];

        var queryColumns = header
            .Where(h => h != "Server" && h != "Database")
            .ToList();

        var columnNames = queryColumns;
        var results = new List<CsvRow>();

        using var reader2 = new StreamReader(filePath, Encoding.UTF8);
        using var csv2 = new CsvHelper.CsvReader(reader2, CultureInfo.InvariantCulture);
        csv2.Read();
        csv2.ReadHeader();

        while (csv2.Read())
        {
            var server = csv2.GetField("Server");
            var database = csv2.GetField("Database");
            var dataRow = new Dictionary<string, string>();
            foreach (var col in queryColumns)
            {
                dataRow[col] = csv2.TryGetField<string>(col, out var val) ? val ?? string.Empty : string.Empty;
            }
            results.Add(new CsvRow(server ?? string.Empty, database ?? string.Empty, DateTime.MinValue, "Success", 0, string.Empty, 0, columnNames, [dataRow]));
        }

        return results;
    }
}
