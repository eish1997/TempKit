param([string]$ReportPath='desktop/evidence/generation/g0/actual-model-discovery.json')
$ErrorActionPreference='Stop'
$credential=Read-Host 'New API token (hidden)' -AsSecureString
$handler=[System.Net.Http.HttpClientHandler]::new()
$handler.AllowAutoRedirect=$false
$handler.UseCookies=$false
$handler.UseDefaultCredentials=$false
$handler.UseProxy=$false
$client=[System.Net.Http.HttpClient]::new($handler)
$client.Timeout=[TimeSpan]::FromSeconds(15)
$client.MaxResponseContentBufferSize=1048576
try{
 $plain=[System.Net.NetworkCredential]::new('', $credential).Password
 $request=[System.Net.Http.HttpRequestMessage]::new([System.Net.Http.HttpMethod]::Get,'http://127.0.0.1:3000/v1/models')
 $request.Headers.Authorization=[System.Net.Http.Headers.AuthenticationHeaderValue]::new('Bearer',$plain)
 $plain=$null
 $response=$client.SendAsync($request).GetAwaiter().GetResult()
 $report=[ordered]@{checkedUtc=[DateTime]::UtcNow.ToString('o');origin='http://127.0.0.1:3000';endpoint='/v1/models';method='GET';status=[int]$response.StatusCode;paidCalls=0;models=@()}
 if($response.IsSuccessStatusCode){
  $body=$response.Content.ReadAsStringAsync().GetAwaiter().GetResult()|ConvertFrom-Json
  $report.models=@($body.data|ForEach-Object {if($_.id -is [string] -and $_.id.Length -le 256){$_.id}})
 }
 $report|ConvertTo-Json -Depth 4|Set-Content -LiteralPath $ReportPath -Encoding utf8
 $report|ConvertTo-Json -Depth 4
}catch{
 Write-Output 'Read-only discovery failed; credential and response body omitted.'
 exit 1
}finally{
 if($request){$request.Dispose()};if($response){$response.Dispose()};$client.Dispose();$handler.Dispose();$credential.Dispose();$plain=$null
}
