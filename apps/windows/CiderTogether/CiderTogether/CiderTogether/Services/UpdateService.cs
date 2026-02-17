using System.Net.Http;
using System.Net.Http.Json;
using System.Runtime.InteropServices;
using System.Text.Json.Serialization;
using Windows.ApplicationModel;
using Windows.Management.Deployment;

namespace CiderTogether.Services;

public enum InstallSource
{
    Sideload,
    Winget,
    Scoop
}

public sealed class UpdateService
{
    private const string GitHubApiUrl =
        "https://api.github.com/repos/giorgiobrullo/cider-listen-together/releases/latest";

    private static readonly Lazy<UpdateService> _instance = new(() => new UpdateService());
    public static UpdateService Instance => _instance.Value;

    private readonly HttpClient _httpClient;

    /// <summary>
    /// How the app was installed. Determines whether self-update is available.
    /// </summary>
    public InstallSource InstallSource { get; }

    /// <summary>
    /// Whether the app can check for updates (not managed by a package manager).
    /// </summary>
    public bool CanCheckForUpdates => InstallSource == InstallSource.Sideload;

    /// <summary>
    /// Display name for the managing package manager, or null if sideloaded.
    /// </summary>
    public string? ManagedByName => InstallSource switch
    {
        InstallSource.Winget => "winget",
        InstallSource.Scoop => "Scoop",
        _ => null
    };

    private UpdateService()
    {
        _httpClient = new HttpClient();
        _httpClient.DefaultRequestHeaders.UserAgent.ParseAdd("CiderTogether");
        InstallSource = DetectInstallSource();
    }

    /// <summary>
    /// Gets the current app version from the MSIX package.
    /// </summary>
    public static Version GetCurrentVersion()
    {
        var pv = Package.Current.Id.Version;
        return new Version(pv.Major, pv.Minor, pv.Build, pv.Revision);
    }

    /// <summary>
    /// Checks GitHub releases for a newer version.
    /// Returns (updateAvailable, latestVersion, downloadUrl).
    /// </summary>
    public async Task<(bool UpdateAvailable, string LatestVersion, string DownloadUrl)> CheckForUpdateAsync()
    {
        try
        {
            var release = await _httpClient.GetFromJsonAsync<GitHubRelease>(GitHubApiUrl);
            if (release?.TagName is null)
                return (false, string.Empty, string.Empty);

            var latestVersion = new Version(release.TagName.TrimStart('v'));
            var currentVersion = GetCurrentVersion();

            if (latestVersion > currentVersion)
            {
                // Find the MSIX asset
                var msixAsset = release.Assets?.FirstOrDefault(
                    a => a.Name.EndsWith(".msix", StringComparison.OrdinalIgnoreCase));

                var downloadUrl = msixAsset?.BrowserDownloadUrl ?? release.HtmlUrl ?? string.Empty;
                return (true, release.TagName.TrimStart('v'), downloadUrl);
            }

            return (false, release.TagName.TrimStart('v'), string.Empty);
        }
        catch (Exception ex)
        {
            System.Diagnostics.Debug.WriteLine($"Update check failed: {ex.Message}");
            return (false, string.Empty, string.Empty);
        }
    }

    /// <summary>
    /// Downloads and installs the MSIX update. The app will shut down and relaunch automatically.
    /// </summary>
    /// <param name="downloadUrl">The MSIX download URL from GitHub releases.</param>
    /// <param name="progress">Optional progress callback (0.0 to 1.0).</param>
    public async Task DownloadAndInstallAsync(string downloadUrl, IProgress<double>? progress = null)
    {
        // Download the MSIX to a temp file
        var tempPath = Path.Combine(Path.GetTempPath(), $"CiderTogether_update_{Guid.NewGuid():N}.msix");

        try
        {
            using var response = await _httpClient.GetAsync(downloadUrl, HttpCompletionOption.ResponseHeadersRead);
            response.EnsureSuccessStatusCode();

            var totalBytes = response.Content.Headers.ContentLength ?? -1;
            var downloadedBytes = 0L;

            using var contentStream = await response.Content.ReadAsStreamAsync();
            using var fileStream = File.Create(tempPath);
            var buffer = new byte[81920];
            int bytesRead;

            while ((bytesRead = await contentStream.ReadAsync(buffer)) > 0)
            {
                await fileStream.WriteAsync(buffer.AsMemory(0, bytesRead));
                downloadedBytes += bytesRead;

                if (totalBytes > 0)
                {
                    progress?.Report((double)downloadedBytes / totalBytes);
                }
            }

            await fileStream.FlushAsync();
            fileStream.Close();

            // Register for restart so the app relaunches after the update
            RegisterApplicationRestart(null, 0);

            // Install the MSIX package — this will shut down the current app
            var packageManager = new PackageManager();
            var deploymentResult = await packageManager.AddPackageAsync(
                new Uri(tempPath),
                null,
                DeploymentOptions.ForceApplicationShutdown);

            if (!string.IsNullOrEmpty(deploymentResult.ErrorText))
            {
                throw new InvalidOperationException(deploymentResult.ErrorText);
            }
        }
        finally
        {
            // Clean up temp file if install failed (if it succeeded, we're already shut down)
            try { File.Delete(tempPath); } catch { }
        }
    }

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
    private static extern uint RegisterApplicationRestart(string? commandLine, int flags);

    private static InstallSource DetectInstallSource()
    {
        // Check for winget: winget-installed MSIX packages can sometimes be detected
        // via registry entries with WinGetInstallerType markers
        if (IsInstalledViaWinget())
            return InstallSource.Winget;

        // Scoop installs to ~/scoop/apps/ - check process path
        if (IsInstalledViaScoop())
            return InstallSource.Scoop;

        return InstallSource.Sideload;
    }

    private static bool IsInstalledViaWinget()
    {
        try
        {
            // Winget records MSIX installations in the registry under Uninstall keys
            string[] registryPaths =
            [
                @"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
                @"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"
            ];

            var packageFamilyName = Package.Current.Id.FamilyName;

            foreach (var path in registryPaths)
            {
                using var key = Microsoft.Win32.Registry.LocalMachine.OpenSubKey(path);
                if (key is null) continue;

                foreach (var subKeyName in key.GetSubKeyNames())
                {
                    using var subKey = key.OpenSubKey(subKeyName);
                    if (subKey?.GetValue("WinGetInstallerType") is not null)
                    {
                        // Check if this entry matches our package
                        var displayName = subKey.GetValue("DisplayName") as string;
                        if (displayName?.Contains("CiderTogether", StringComparison.OrdinalIgnoreCase) == true
                            || displayName?.Contains("Cider Together", StringComparison.OrdinalIgnoreCase) == true)
                        {
                            return true;
                        }
                    }
                }
            }
        }
        catch
        {
            // Registry access may fail - not installed via winget
        }

        return false;
    }

    private static bool IsInstalledViaScoop()
    {
        try
        {
            var processPath = Environment.ProcessPath;
            if (string.IsNullOrEmpty(processPath)) return false;

            var scoopDir = Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
                "scoop", "apps");

            return processPath.Contains(scoopDir, StringComparison.OrdinalIgnoreCase);
        }
        catch
        {
            return false;
        }
    }

    private record GitHubRelease
    {
        [JsonPropertyName("tag_name")]
        public string? TagName { get; init; }

        [JsonPropertyName("html_url")]
        public string? HtmlUrl { get; init; }

        [JsonPropertyName("assets")]
        public GitHubAsset[]? Assets { get; init; }
    }

    private record GitHubAsset
    {
        [JsonPropertyName("name")]
        public string Name { get; init; } = "";

        [JsonPropertyName("browser_download_url")]
        public string BrowserDownloadUrl { get; init; } = "";
    }
}
