import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { chmod, cp, mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const manifest=JSON.parse(await readFile(path.join(root,'src-tauri/moli-release.json'),'utf8'));
async function hash(file){const digest=createHash('sha256');for await(const chunk of createReadStream(file))digest.update(chunk);return digest.digest('hex');}
const targets=process.platform==='darwin'?['darwin-arm64','darwin-x64']:[`${process.platform}-${process.arch}`];
for(const target of targets){
  const asset=manifest.assets[target];if(!asset)throw Error(`Moli is not packaged for ${target}`);
  const binary=target.startsWith('win32')?'moli.exe':'moli';
  const destination=path.join(root,'src-tauri/resources/moli',target);
  try{const installed=JSON.parse(await readFile(path.join(destination,'verified.json'),'utf8'));if(installed.version===manifest.version&&installed.archiveSha256===asset[1]&&installed.binarySha256===await hash(path.join(destination,binary)))continue;}catch{/* Acquire the pinned artifact. */}
  const cache=path.join(root,'.moli-cache',manifest.version,target);await mkdir(cache,{recursive:true});
  const archive=path.join(cache,asset[0]);
  let valid=false;try{valid=await hash(archive)===asset[1];}catch{}
  if(!valid){
    const response=await fetch(`https://github.com/lexmount/moli/releases/download/v${manifest.version}/${asset[0]}`);
    if(!response.ok)throw Error(`Moli download failed: HTTP ${response.status}`);
    await writeFile(archive,Buffer.from(await response.arrayBuffer()));
  }
  if(await hash(archive)!==asset[1])throw Error(`Moli archive digest mismatch: ${target}`);
  const extracted=path.join(cache,'extracted');await mkdir(extracted,{recursive:true});
  if(process.platform==='win32')execFileSync('powershell.exe',['-NoProfile','-File',path.join(root,'scripts/extract-moli.ps1'),'-Archive',archive,'-Destination',extracted],{windowsHide:true,stdio:'pipe'});
  else execFileSync('tar',['-xzf',archive,'-C',extracted],{stdio:'pipe'});
  async function find(dir){for(const entry of await readdir(dir,{withFileTypes:true})){if(entry.isFile()&&entry.name===binary)return dir;if(entry.isDirectory()){const nested=await find(path.join(dir,entry.name));if(nested)return nested;}}}
  const packageRoot=await find(extracted);if(!packageRoot)throw Error('Moli executable missing from verified archive');
  await mkdir(destination,{recursive:true});await cp(packageRoot,destination,{recursive:true});
  if(process.platform!=='win32')await chmod(path.join(destination,binary),0o755);
  await writeFile(path.join(destination,'verified.json'),JSON.stringify({version:manifest.version,archiveSha256:asset[1],binarySha256:await hash(path.join(destination,binary))},null,2));
  console.log(`Bundled verified Moli ${manifest.version} (${target})`);
}
