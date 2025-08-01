# Binance连接器测试运行脚本
# 这个脚本提供了运行不同类型测试的便捷方式

param(
    [string]$TestType = "unit",
    [switch]$Verbose,
    [switch]$SetupEnv
)

# 颜色输出函数
function Write-ColorOutput($ForegroundColor) {
    $fc = $host.UI.RawUI.ForegroundColor
    $host.UI.RawUI.ForegroundColor = $ForegroundColor
    if ($args) {
        Write-Output $args
    } else {
        $input | Write-Output
    }
    $host.UI.RawUI.ForegroundColor = $fc
}

function Write-Success { Write-ColorOutput Green $args }
function Write-Warning { Write-ColorOutput Yellow $args }
function Write-Error { Write-ColorOutput Red $args }
function Write-Info { Write-ColorOutput Cyan $args }

# 显示帮助信息
function Show-Help {
    Write-Info "Binance连接器测试运行脚本"
    Write-Info "=" * 40
    Write-Output ""
    Write-Output "用法: .\run_tests.ps1 [选项]"
    Write-Output ""
    Write-Output "选项:"
    Write-Output "  -TestType <type>    指定测试类型 (unit|integration|all|performance|stress)"
    Write-Output "  -Verbose           显示详细输出"
    Write-Output "  -SetupEnv          设置测试环境变量"
    Write-Output "  -Help              显示此帮助信息"
    Write-Output ""
    Write-Output "测试类型说明:"
    Write-Output "  unit        - 运行单元测试（默认）"
    Write-Output "  integration - 运行集成测试（需要API凭据）"
    Write-Output "  all         - 运行所有测试"
    Write-Output "  performance - 运行性能测试"
    Write-Output "  stress      - 运行压力测试"
    Write-Output ""
    Write-Output "示例:"
    Write-Output "  .\run_tests.ps1                    # 运行单元测试"
    Write-Output "  .\run_tests.ps1 -TestType all      # 运行所有测试"
    Write-Output "  .\run_tests.ps1 -SetupEnv          # 设置环境变量"
    Write-Output "  .\run_tests.ps1 -TestType integration -Verbose  # 运行集成测试并显示详细输出"
}

# 设置环境变量
function Setup-Environment {
    Write-Info "设置Binance测试环境变量"
    Write-Info "=" * 30
    
    $apiKey = Read-Host "请输入Binance API Key (留空使用测试密钥)"
    if ([string]::IsNullOrWhiteSpace($apiKey)) {
        $apiKey = "dummy_key"
    }
    
    $secretKey = Read-Host "请输入Binance Secret Key (留空使用测试密钥)"
    if ([string]::IsNullOrWhiteSpace($secretKey)) {
        $secretKey = "dummy_secret"
    }
    
    $useTestnet = Read-Host "使用测试网? (y/N)"
    $testnet = if ($useTestnet -eq "y" -or $useTestnet -eq "Y") { "true" } else { "false" }
    
    # 设置环境变量
    $env:BINANCE_API_KEY = $apiKey
    $env:BINANCE_SECRET_KEY = $secretKey
    $env:BINANCE_TESTNET = $testnet
    
    Write-Success "环境变量设置完成:"
    Write-Output "  BINANCE_API_KEY: $($apiKey.Substring(0, [Math]::Min(8, $apiKey.Length)))..."
    Write-Output "  BINANCE_SECRET_KEY: $($secretKey.Substring(0, [Math]::Min(8, $secretKey.Length)))..."
    Write-Output "  BINANCE_TESTNET: $testnet"
    Write-Output ""
}

# 检查Rust环境
function Test-RustEnvironment {
    Write-Info "检查Rust环境..."
    
    try {
        $cargoVersion = cargo --version 2>$null
        if ($LASTEXITCODE -eq 0) {
            Write-Success "✅ Cargo: $cargoVersion"
        } else {
            Write-Error "❌ Cargo未安装或不在PATH中"
            return $false
        }
    } catch {
        Write-Error "❌ 无法检查Cargo版本"
        return $false
    }
    
    return $true
}

# 运行编译检查
function Invoke-CompileCheck {
    Write-Info "运行编译检查..."
    
    $result = cargo check 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Success "✅ 编译检查通过"
        if ($Verbose) {
            Write-Output $result
        }
        return $true
    } else {
        Write-Error "❌ 编译检查失败:"
        Write-Output $result
        return $false
    }
}

# 运行单元测试
function Invoke-UnitTests {
    Write-Info "运行单元测试..."
    
    $testArgs = @("test", "--lib")
    if ($Verbose) {
        $testArgs += "--", "--nocapture"
    }
    
    $result = & cargo @testArgs 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Success "✅ 单元测试通过"
        if ($Verbose) {
            Write-Output $result
        }
        return $true
    } else {
        Write-Error "❌ 单元测试失败:"
        Write-Output $result
        return $false
    }
}

# 运行集成测试
function Invoke-IntegrationTests {
    Write-Info "运行集成测试..."
    
    # 检查是否有API凭据
    $hasCredentials = $env:BINANCE_API_KEY -and $env:BINANCE_SECRET_KEY -and 
                     $env:BINANCE_API_KEY -ne "dummy_key" -and $env:BINANCE_SECRET_KEY -ne "dummy_secret"
    
    if (-not $hasCredentials) {
        Write-Warning "⚠️  未检测到有效的API凭据，集成测试可能会跳过某些功能"
        Write-Warning "   使用 -SetupEnv 参数设置API凭据"
    }
    
    $testArgs = @("test", "--test", "binance_integration_tests")
    if ($Verbose) {
        $testArgs += "--", "--nocapture"
    }
    
    $result = & cargo @testArgs 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Success "✅ 集成测试通过"
        if ($Verbose) {
            Write-Output $result
        }
        return $true
    } else {
        Write-Error "❌ 集成测试失败:"
        Write-Output $result
        return $false
    }
}

# 运行性能测试
function Invoke-PerformanceTests {
    Write-Info "运行性能测试..."
    
    $testArgs = @("test", "performance_tests", "--release")
    if ($Verbose) {
        $testArgs += "--", "--nocapture"
    }
    
    $result = & cargo @testArgs 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Success "✅ 性能测试通过"
        if ($Verbose) {
            Write-Output $result
        }
        return $true
    } else {
        Write-Error "❌ 性能测试失败:"
        Write-Output $result
        return $false
    }
}

# 运行压力测试
function Invoke-StressTests {
    Write-Info "运行压力测试..."
    
    $testArgs = @("test", "stress_tests", "--release")
    if ($Verbose) {
        $testArgs += "--", "--nocapture"
    }
    
    $result = & cargo @testArgs 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Success "✅ 压力测试通过"
        if ($Verbose) {
            Write-Output $result
        }
        return $true
    } else {
        Write-Error "❌ 压力测试失败:"
        Write-Output $result
        return $false
    }
}

# 主函数
function Main {
    param($TestType, $Verbose, $SetupEnv)
    
    Write-Info "🚀 Binance连接器测试套件"
    Write-Info "=" * 40
    Write-Output ""
    
    # 显示帮助
    if ($args -contains "-Help" -or $args -contains "--help" -or $args -contains "-h") {
        Show-Help
        return
    }
    
    # 设置环境变量
    if ($SetupEnv) {
        Setup-Environment
    }
    
    # 检查Rust环境
    if (-not (Test-RustEnvironment)) {
        Write-Error "Rust环境检查失败，请确保已安装Rust和Cargo"
        return 1
    }
    
    Write-Output ""
    
    # 运行编译检查
    if (-not (Invoke-CompileCheck)) {
        Write-Error "编译检查失败，请修复编译错误后重试"
        return 1
    }
    
    Write-Output ""
    
    # 根据测试类型运行相应测试
    $success = $true
    
    switch ($TestType.ToLower()) {
        "unit" {
            $success = Invoke-UnitTests
        }
        "integration" {
            $success = Invoke-IntegrationTests
        }
        "performance" {
            $success = Invoke-PerformanceTests
        }
        "stress" {
            $success = Invoke-StressTests
        }
        "all" {
            Write-Info "运行所有测试..."
            Write-Output ""
            
            $success = $success -and (Invoke-UnitTests)
            Write-Output ""
            
            $success = $success -and (Invoke-IntegrationTests)
            Write-Output ""
            
            $success = $success -and (Invoke-PerformanceTests)
            Write-Output ""
            
            $success = $success -and (Invoke-StressTests)
        }
        default {
            Write-Error "未知的测试类型: $TestType"
            Write-Output "支持的测试类型: unit, integration, performance, stress, all"
            Show-Help
            return 1
        }
    }
    
    Write-Output ""
    Write-Info "=" * 40
    
    if ($success) {
        Write-Success "🎉 所有测试通过！"
        return 0
    } else {
        Write-Error "❌ 测试失败"
        return 1
    }
}

# 运行主函数
exit (Main -TestType $TestType -Verbose $Verbose -SetupEnv $SetupEnv)