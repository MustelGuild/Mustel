using System;
using System.IO;
using System.Linq;

using FurLab.Core.Interfaces;
using FurLab.Core.Models;
using FurLab.Core.Services;

using Microsoft.VisualStudio.TestTools.UnitTesting;

namespace FurLab.Tests.Commands;

/// <summary>
/// Unit tests for PgPassService (US1, US2, US3).
/// Following TDD: tests written before implementation.
/// </summary>
[TestClass]
public class PgPassServiceTests
{
    private IPgPassService _service = null!;
    private string _tempDir = null!;
    private string _tempFile = null!;

    [TestInitialize]
    public void TestInitialize()
    {
        _service = new PgPassService();
        _tempDir = Path.Combine(Path.GetTempPath(), $"pgpass_test_{Guid.NewGuid():N}");
        _tempFile = Path.Combine(_tempDir, "pgpass.conf");
    }

    [TestCleanup]
    public void TestCleanup()
    {
        if (Directory.Exists(_tempDir))
        {
            Directory.Delete(_tempDir, recursive: true);
        }
    }

    // =========================================================================
    // US1 — AddEntry
    // =========================================================================

    [TestMethod(DisplayName = "AddEntry with non-existent directory should create directory automatically")]
    [Description("Verifies that the AddEntry method creates the parent directory of the pgpass.conf file when it does not exist.")]
    [TestCategory("US1")]
    public void AddEntry_NonExistentDirectory_CreatesDirectoryAutomatically()
    {
        var entry = new PgPassEntry
        {
            Hostname = "localhost",
            Port = "5432",
            Database = "meu_banco",
            Username = "postgres",
            Password = "senha123"
        };

        var result = _service.AddEntry(entry, _tempFile);

        Assert.IsTrue(result.Success, result.Message);
        Assert.IsTrue(Directory.Exists(_tempDir), "Directory should be created automatically.");
        Assert.IsTrue(File.Exists(_tempFile), "pgpass.conf file should be created.");
    }

    [TestMethod(DisplayName = "AddEntry should record entry in host:port:database:username:password format")]
    [Description("Verifies that the entry is written to the file following the standard pgpass format.")]
    [TestCategory("US1")]
    public void AddEntry_ValidEntry_RecordsCorrectFormat()
    {
        var entry = new PgPassEntry
        {
            Hostname = "localhost",
            Port = "5432",
            Database = "meu_banco",
            Username = "postgres",
            Password = "senha123"
        };

        _service.AddEntry(entry, _tempFile);

        var conteudo = File.ReadAllText(_tempFile);
        Assert.IsTrue(conteudo.Contains("localhost:5432:meu_banco:postgres:senha123"),
            $"Expected format not found. Content: {conteudo}");
    }

    [TestMethod(DisplayName = "AddEntry should preserve existing entries when adding a new one")]
    [Description("Verifies that adding a new entry does not remove entries already present in the file.")]
    [TestCategory("US1")]
    public void AddEntry_NewEntry_PreservesExisting()
    {
        Directory.CreateDirectory(_tempDir);
        File.WriteAllText(_tempFile, "localhost:5432:banco_existente:postgres:senha_antiga\n");

        var novaEntrada = new PgPassEntry
        {
            Hostname = "localhost",
            Port = "5432",
            Database = "banco_novo",
            Username = "postgres",
            Password = "senha_nova"
        };

        _service.AddEntry(novaEntrada, _tempFile);

        var linhas = File.ReadAllLines(_tempFile);
        Assert.IsTrue(linhas.Any(l => l.Contains("banco_existente")),
            "Existing entry should be preserved.");
        Assert.IsTrue(linhas.Any(l => l.Contains("banco_novo")),
            "New entry should be added.");
    }

    [TestMethod(DisplayName = "AddEntry with duplicate entry should return IsDuplicate=true")]
    [Description("Verifies that when adding an entry identical to an existing one, the result indicates a duplicate without error.")]
    [TestCategory("US1")]
    public void AddEntry_DuplicateEntry_ReturnsIsDuplicateTrue()
    {
        var entry = new PgPassEntry
        {
            Hostname = "localhost",
            Port = "5432",
            Database = "meu_banco",
            Username = "postgres",
            Password = "senha123"
        };

        _service.AddEntry(entry, _tempFile);
        var resultado = _service.AddEntry(entry, _tempFile);

        Assert.IsTrue(resultado.Success, "Duplicate is not an error — Success should be true.");
        Assert.IsTrue(resultado.IsDuplicate, "IsDuplicate should be true for duplicate entry.");

        var linhas = File.ReadAllLines(_tempFile);
        Assert.AreEqual(1, linhas.Length, "Only one line should exist in the file.");
    }

    [TestMethod(DisplayName = "AddEntry should escape colons in the password")]
    [Description("Verifies that ':' characters in the password are escaped with '\\' according to the pgpass format.")]
    [TestCategory("US1")]
    public void AddEntry_PasswordWithColons_EscapesCorrectly()
    {
        var entry = new PgPassEntry
        {
            Hostname = "localhost",
            Port = "5432",
            Database = "meu_banco",
            Username = "postgres",
            Password = "senha:com:dois:pontos"
        };

        _service.AddEntry(entry, _tempFile);

        var conteudo = File.ReadAllText(_tempFile);
        Assert.IsTrue(conteudo.Contains(@"senha\:com\:dois\:pontos"),
            $"Colons must be escaped. Content: {conteudo}");
    }

    [TestMethod(DisplayName = "AddEntry should escape backslash in the password")]
    [Description("Verifies that '\\' characters in the password are escaped with an additional '\\' according to the pgpass format.")]
    [TestCategory("US1")]
    public void AddEntry_PasswordWithBackslash_EscapesCorrectly()
    {
        var entry = new PgPassEntry
        {
            Hostname = "localhost",
            Port = "5432",
            Database = "meu_banco",
            Username = "postgres",
            Password = @"senha\invertida"
        };

        _service.AddEntry(entry, _tempFile);

        var conteudo = File.ReadAllText(_tempFile);
        Assert.IsTrue(conteudo.Contains(@"senha\\invertida"),
            $"Backslashes must be escaped. Content: {conteudo}");
    }

    [TestMethod(DisplayName = "AddEntry with empty password should fail without creating file")]
    [Description("Verifies that entries with empty passwords are rejected and no file is created.")]
    [TestCategory("US1")]
    public void AddEntry_EmptyPassword_FailsWithoutCreatingFile()
    {
        var entry = new PgPassEntry
        {
            Hostname = "localhost",
            Port = "5432",
            Database = "meu_banco",
            Username = "postgres",
            Password = ""
        };

        var resultado = _service.AddEntry(entry, _tempFile);

        Assert.IsFalse(resultado.Success, "Empty password should result in failure.");
        Assert.IsFalse(File.Exists(_tempFile), "File should not be created with empty password.");
    }

    [TestMethod(DisplayName = "AddEntry with omitted parameters should apply default values")]
    [Description("Verifies that fields not provided in the entry receive the default values from the PgPassEntry record.")]
    [TestCategory("US1")]
    public void AddEntry_OmittedParameters_AppliesDefaultValues()
    {
        var entry = new PgPassEntry
        {
            Database = "meu_banco",
            Password = "senha123"
            // Hostname, Port e Username using default values from record
        };

        _service.AddEntry(entry, _tempFile);

        var conteudo = File.ReadAllText(_tempFile);
        Assert.IsTrue(conteudo.Contains("localhost:5432:meu_banco:postgres:senha123"),
            $"Defaults should be applied. Content: {conteudo}");
    }

    [TestMethod(DisplayName = "AddEntry with database '*' should record wildcard literally")]
    [Description("Verifies that the '*' character in the Database field is written without escaping in the pgpass file.")]
    [TestCategory("US1")]
    public void AddEntry_DatabaseWithWildcard_RecordsLiterally()
    {
        var entry = new PgPassEntry
        {
            Hostname = "localhost",
            Port = "5432",
            Database = "*",
            Username = "postgres",
            Password = "senha123"
        };

        _service.AddEntry(entry, _tempFile);

        var conteudo = File.ReadAllText(_tempFile);
        Assert.IsTrue(conteudo.Contains("localhost:5432:*:postgres:senha123"),
            $"Wildcard '*' must be written literally. Content: {conteudo}");
    }

    // =========================================================================
    // US2 — ListEntries
    // =========================================================================

    [TestMethod(DisplayName = "ListEntries with non-existent file should return empty list")]
    [Description("Verifies that the entry listing returns an empty collection when the pgpass file does not exist.")]
    [TestCategory("US2")]
    public void ListEntries_NonExistentFile_ReturnsEmptyList()
    {
        var entradas = _service.ListEntries(_tempFile).ToList();

        Assert.AreEqual(0, entradas.Count, "Should return empty list when file does not exist.");
    }

    [TestMethod(DisplayName = "ListEntries with empty file should return empty list")]
    [Description("Verifies that the entry listing returns an empty collection when the pgpass file exists but is empty.")]
    [TestCategory("US2")]
    public void ListEntries_EmptyFile_ReturnsEmptyList()
    {
        Directory.CreateDirectory(_tempDir);
        File.WriteAllText(_tempFile, string.Empty);

        var entradas = _service.ListEntries(_tempFile).ToList();

        Assert.AreEqual(0, entradas.Count, "Should return empty list when file is empty.");
    }

    [TestMethod(DisplayName = "ListEntries should ignore comment lines")]
    [Description("Verifies that lines starting with '#' are filtered during pgpass file parsing.")]
    [TestCategory("US2")]
    public void ListEntries_CommentLines_AreIgnored()
    {
        Directory.CreateDirectory(_tempDir);
        File.WriteAllLines(_tempFile, new[]
        {
            "# Example comment",
            "localhost:5432:meu_banco:postgres:senha123",
            "# Another comment"
        });

        var entradas = _service.ListEntries(_tempFile).ToList();

        Assert.AreEqual(1, entradas.Count, "Comment lines should be ignored.");
    }

    [TestMethod(DisplayName = "ListEntries should return all entries correctly parsed")]
    [Description("Verifies that each valid line of the file is converted into a PgPassEntry with the correct fields.")]
    [TestCategory("US2")]
    public void ListEntries_MultipleEntries_ReturnsAllParsed()
    {
        Directory.CreateDirectory(_tempDir);
        File.WriteAllLines(_tempFile, new[]
        {
            "localhost:5432:banco1:postgres:senha1",
            "db.empresa.com:5433:banco2:deploy:senha2"
        });

        var entradas = _service.ListEntries(_tempFile).ToList();

        Assert.AreEqual(2, entradas.Count);
        Assert.AreEqual("localhost", entradas[0].Hostname);
        Assert.AreEqual("5432", entradas[0].Port);
        Assert.AreEqual("banco1", entradas[0].Database);
        Assert.AreEqual("postgres", entradas[0].Username);
        Assert.AreEqual("senha1", entradas[0].Password);

        Assert.AreEqual("db.empresa.com", entradas[1].Hostname);
        Assert.AreEqual("5433", entradas[1].Port);
        Assert.AreEqual("banco2", entradas[1].Database);
        Assert.AreEqual("deploy", entradas[1].Username);
        Assert.AreEqual("senha2", entradas[1].Password);
    }

    [TestMethod(DisplayName = "ListEntries should apply unescape to password when reading")]
    [Description("Verifies that escape sequences '\\:' and '\\\\' are converted back to ':' and '\\' respectively.")]
    [TestCategory("US2")]
    public void ListEntries_PasswordWithEscape_AppliesUnescape()
    {
        Directory.CreateDirectory(_tempDir);
        // File contains entries with escapes applied
        File.WriteAllLines(_tempFile, new[]
        {
            @"localhost:5432:meu_banco:postgres:senha\:com\:dois\:pontos",
            @"localhost:5432:outro_banco:postgres:senha\\invertida"
        });

        var entradas = _service.ListEntries(_tempFile).ToList();

        Assert.AreEqual("senha:com:dois:pontos", entradas[0].Password,
            "Should apply unescape from \\: → :");
        Assert.AreEqual(@"senha\invertida", entradas[1].Password,
            @"Should apply unescape from \\ → \");
    }

    // =========================================================================
    // US3 — RemoveEntry
    // =========================================================================

    [TestMethod(DisplayName = "RemoveEntry should remove correct entry and preserve others")]
    [Description("Verifies that removal by key (host:port:database:username) eliminates only the corresponding entry.")]
    [TestCategory("US3")]
    public void RemoveEntry_ExistingEntry_RemovesWhilePreservingOthers()
    {
        Directory.CreateDirectory(_tempDir);
        File.WriteAllLines(_tempFile, new[]
        {
            "localhost:5432:banco1:postgres:senha1",
            "localhost:5432:banco2:postgres:senha2",
            "localhost:5432:banco3:postgres:senha3"
        });

        var chave = new PgPassEntry
        {
            Hostname = "localhost",
            Port = "5432",
            Database = "banco2",
            Username = "postgres",
            Password = "qualquer"
        };

        var resultado = _service.RemoveEntry(chave, _tempFile);

        Assert.IsTrue(resultado.Success, resultado.Message);
        var linhas = File.ReadAllLines(_tempFile);
        Assert.AreEqual(2, linhas.Length, "Only the removed entry should disappear.");
        Assert.IsFalse(linhas.Any(l => l.Contains("banco2")), "banco2 should be removed.");
        Assert.IsTrue(linhas.Any(l => l.Contains("banco1")), "banco1 should be preserved.");
        Assert.IsTrue(linhas.Any(l => l.Contains("banco3")), "banco3 should be preserved.");
    }

    [TestMethod(DisplayName = "RemoveEntry with non-existent entry should return informative result")]
    [Description("Verifies that attempting to remove an entry that does not exist returns a result with Success=false and a descriptive message.")]
    [TestCategory("US3")]
    public void RemoveEntry_NonExistentEntry_ReturnsInformativeResult()
    {
        Directory.CreateDirectory(_tempDir);
        File.WriteAllLines(_tempFile, new[]
        {
            "localhost:5432:banco1:postgres:senha1"
        });

        var chave = new PgPassEntry
        {
            Hostname = "localhost",
            Port = "5432",
            Database = "banco_inexistente",
            Username = "postgres",
            Password = "qualquer"
        };

        var resultado = _service.RemoveEntry(chave, _tempFile);

        Assert.IsFalse(resultado.Success, "Should return informative failure when entry does not exist.");
        var linhas = File.ReadAllLines(_tempFile);
        Assert.AreEqual(1, linhas.Length, "File should remain unchanged.");
    }

    [TestMethod(DisplayName = "RemoveEntry with non-existent entry should leave file unchanged")]
    [Description("Verifies that attempting to remove an entry that does not exist does not modify the file content.")]
    [TestCategory("US3")]
    public void RemoveEntry_NonExistentEntry_FileUnchanged()
    {
        Directory.CreateDirectory(_tempDir);
        var conteudoOriginal = "localhost:5432:banco1:postgres:senha1\n";
        File.WriteAllText(_tempFile, conteudoOriginal);

        var chave = new PgPassEntry
        {
            Hostname = "remotehost",
            Port = "5432",
            Database = "banco1",
            Username = "postgres",
            Password = "qualquer"
        };

        _service.RemoveEntry(chave, _tempFile);

        var conteudoAtual = File.ReadAllText(_tempFile);
        Assert.AreEqual(conteudoOriginal, conteudoAtual,
            "File content should not be changed when entry is not found.");
    }

    // =========================================================================
    // pgpass wildcards — support for * in host, port, database, and username
    // =========================================================================

    [TestMethod(DisplayName = "AddEntry with wildcard in host should record correct entry")]
    [Description("Verifies that the '*' character in the Hostname field is accepted and recorded literally.")]
    [TestCategory("pgpass-wildcard")]
    public void AddEntry_WildcardInHost_RecordsCorrectEntry()
    {
        var entry = new PgPassEntry
        {
            Hostname = "*",
            Port = "5432",
            Database = "meu_banco",
            Username = "postgres",
            Password = "senha123"
        };

        var result = _service.AddEntry(entry, _tempFile);

        Assert.IsTrue(result.Success, result.Message);
        var conteudo = File.ReadAllText(_tempFile);
        Assert.IsTrue(conteudo.Contains("*:5432:meu_banco:postgres:senha123"),
            "Entry with wildcard in host should be recorded correctly.");
    }

    [TestMethod(DisplayName = "AddEntry with wildcard in port should record correct entry")]
    [Description("Verifies that the '*' character in the Port field is accepted and recorded literally.")]
    [TestCategory("pgpass-wildcard")]
    public void AddEntry_WildcardInPort_RecordsCorrectEntry()
    {
        var entry = new PgPassEntry
        {
            Hostname = "localhost",
            Port = "*",
            Database = "meu_banco",
            Username = "postgres",
            Password = "senha123"
        };

        var result = _service.AddEntry(entry, _tempFile);

        Assert.IsTrue(result.Success, result.Message);
        var conteudo = File.ReadAllText(_tempFile);
        Assert.IsTrue(conteudo.Contains("localhost:*:meu_banco:postgres:senha123"),
            "Entry with wildcard in port should be recorded correctly.");
    }

    [TestMethod(DisplayName = "AddEntry with wildcard in username should record correct entry")]
    [Description("Verifies that the '*' character in the Username field is accepted and recorded literally.")]
    [TestCategory("pgpass-wildcard")]
    public void AddEntry_WildcardInUsername_RecordsCorrectEntry()
    {
        var entry = new PgPassEntry
        {
            Hostname = "localhost",
            Port = "5432",
            Database = "meu_banco",
            Username = "*",
            Password = "senha123"
        };

        var result = _service.AddEntry(entry, _tempFile);

        Assert.IsTrue(result.Success, result.Message);
        var conteudo = File.ReadAllText(_tempFile);
        Assert.IsTrue(conteudo.Contains("localhost:5432:meu_banco:*:senha123"),
            "Entry with wildcard in username should be recorded correctly.");
    }

    [TestMethod(DisplayName = "AddEntry with all fields as wildcards should record correct entry")]
    [Description("Verifies that multiple '*' characters in different fields are accepted and recorded literally.")]
    [TestCategory("pgpass-wildcard")]
    public void AddEntry_AllWildcards_RecordsCorrectEntry()
    {
        var entry = new PgPassEntry
        {
            Hostname = "*",
            Port = "*",
            Database = "*",
            Username = "*",
            Password = "senha123"
        };

        var result = _service.AddEntry(entry, _tempFile);

        Assert.IsTrue(result.Success, result.Message);
        var conteudo = File.ReadAllText(_tempFile);
        Assert.IsTrue(conteudo.Contains("*:*:*:*:senha123"),
            "Entry with all wildcards should be recorded correctly.");
    }
}
