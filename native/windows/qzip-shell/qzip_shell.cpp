// Windows 11 Explorer command provider for the QZip flyout.
// It never processes archives in Explorer: selections are written to a bounded,
// one-shot local request file and handed to QZip with a UUID token.
#include <windows.h>
#include <shobjidl_core.h>
#include <shlobj_core.h>
#include <objbase.h>
#include <filesystem>
#include <algorithm>
#include <new>
#include <string>
#include <vector>

namespace {
const CLSID CLSID_QZipExplorerCommand = {0x42e56c7e,0x21cf,0x4ed7,{0x91,0x40,0xb6,0x74,0x11,0x97,0x92,0x41}};
HMODULE g_module = nullptr;
enum class Action { Root, Open, SevenZip, Zip, ExtractHere, ExtractNamed, MoreOptions };

std::wstring CopyString(const wchar_t* value, LPWSTR* output) {
  const size_t count = wcslen(value) + 1;
  auto result = static_cast<LPWSTR>(CoTaskMemAlloc(count * sizeof(wchar_t)));
  if (!result) return L"";
  memcpy(result, value, count * sizeof(wchar_t)); *output = result; return result;
}
const wchar_t* Title(Action action) {
  switch (action) {
    case Action::Open: return L"打开 QZip";
    case Action::SevenZip: return L"压缩为 .7z";
    case Action::Zip: return L"压缩为 .zip";
    case Action::ExtractHere: return L"解压到此处";
    case Action::ExtractNamed: return L"解压到同名文件夹";
    case Action::MoreOptions: return L"更多选项…";
    default: return L"QZip";
  }
}
const wchar_t* ActionName(Action action) {
  switch (action) {
    case Action::Open: return L"open"; case Action::SevenZip: return L"compress-sevenzip";
    case Action::Zip: return L"compress-zip"; case Action::ExtractHere: return L"extract-here";
    case Action::ExtractNamed: return L"extract-named"; default: return L"more-options";
  }
}
std::string Utf8(const std::wstring& value) {
  if (value.empty()) return {};
  int bytes = WideCharToMultiByte(CP_UTF8, 0, value.c_str(), static_cast<int>(value.size()), nullptr, 0, nullptr, nullptr);
  std::string result(bytes, '\0');
  WideCharToMultiByte(CP_UTF8, 0, value.c_str(), static_cast<int>(value.size()), result.data(), bytes, nullptr, nullptr);
  return result;
}
std::string JsonEscape(const std::string& value) {
  std::string out; out.reserve(value.size() + 8);
  for (unsigned char c : value) { if (c == '\\' || c == '"') { out += '\\'; out += c; } else if (c >= 0x20) out += c; }
  return out;
}
bool SaveRequest(Action action, IShellItemArray* items, std::wstring* token) {
  DWORD count = 0; if (!items || FAILED(items->GetCount(&count)) || count == 0 || count > 1000) return false;
  PWSTR local = nullptr; if (FAILED(SHGetKnownFolderPath(FOLDERID_LocalAppData, 0, nullptr, &local))) return false;
  std::filesystem::path root = std::filesystem::path(local) / L"QZip" / L"ShellRequests"; CoTaskMemFree(local);
  std::error_code ec; std::filesystem::create_directories(root, ec); if (ec) return false;
  GUID guid{}; if (FAILED(CoCreateGuid(&guid))) return false; wchar_t guidText[40]{}; StringFromGUID2(guid, guidText, 40);
  *token = guidText; std::filesystem::path request = root / (std::wstring(guidText) + L".json");
  HANDLE file = CreateFileW(request.c_str(), GENERIC_WRITE, 0, nullptr, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, nullptr);
  if (file == INVALID_HANDLE_VALUE) return false;
  std::string body = "{\"action\":\"" + Utf8(ActionName(action)) + "\",\"paths\":[";
  for (DWORD i = 0; i < count; ++i) { IShellItem* item = nullptr; PWSTR path = nullptr; if (SUCCEEDED(items->GetItemAt(i, &item)) && SUCCEEDED(item->GetDisplayName(SIGDN_FILESYSPATH, &path))) { if (body.back() != '[') body += ','; body += "\"" + JsonEscape(Utf8(path)) + "\""; CoTaskMemFree(path); } if (item) item->Release(); }
  body += "]}"; if (body.size() > 4 * 1024 * 1024) { CloseHandle(file); DeleteFileW(request.c_str()); return false; }
  DWORD written = 0; const bool ok = WriteFile(file, body.data(), static_cast<DWORD>(body.size()), &written, nullptr) && written == body.size(); CloseHandle(file);
  if (!ok) DeleteFileW(request.c_str()); return ok;
}
bool LaunchQZip(const std::wstring& token) {
  wchar_t module[MAX_PATH]{}; if (!GetModuleFileNameW(g_module, module, MAX_PATH)) return false;
  // Tauri installs the resource DLL below the application directory; the
  // executable name is the Cargo binary name, not the product display name.
  std::filesystem::path executable = std::filesystem::path(module).parent_path().parent_path() / L"qzip-desktop.exe";
  if (!std::filesystem::exists(executable)) executable = std::filesystem::path(module).parent_path() / L"qzip-desktop.exe";
  if (!std::filesystem::exists(executable)) return false;
  std::wstring command = L"\"" + executable.wstring() + L"\" --shell-request " + token;
  STARTUPINFOW startup{sizeof(startup)}; PROCESS_INFORMATION process{};
  const BOOL started = CreateProcessW(executable.c_str(), command.data(), nullptr, nullptr, FALSE, 0, nullptr, executable.parent_path().c_str(), &startup, &process);
  if (started) { CloseHandle(process.hThread); CloseHandle(process.hProcess); } return started != FALSE;
}

class Command;
class CommandEnumerator final : public IEnumExplorerCommand {
 public: CommandEnumerator(); HRESULT STDMETHODCALLTYPE QueryInterface(REFIID, void**) override; ULONG STDMETHODCALLTYPE AddRef() override; ULONG STDMETHODCALLTYPE Release() override; HRESULT STDMETHODCALLTYPE Next(ULONG, IExplorerCommand**, ULONG*) override; HRESULT STDMETHODCALLTYPE Skip(ULONG) override; HRESULT STDMETHODCALLTYPE Reset() override; HRESULT STDMETHODCALLTYPE Clone(IEnumExplorerCommand**) override;
 ~CommandEnumerator();
 private: LONG refs_ = 1; ULONG index_ = 0; std::vector<Command*> commands_;
};
class Command final : public IExplorerCommand {
 public: explicit Command(Action action) : action_(action) {} HRESULT STDMETHODCALLTYPE QueryInterface(REFIID, void**) override; ULONG STDMETHODCALLTYPE AddRef() override; ULONG STDMETHODCALLTYPE Release() override; HRESULT STDMETHODCALLTYPE GetTitle(IShellItemArray*, LPWSTR* value) override { return CopyString(Title(action_), value).empty() ? E_OUTOFMEMORY : S_OK; } HRESULT STDMETHODCALLTYPE GetIcon(IShellItemArray*, LPWSTR*) override { return E_NOTIMPL; } HRESULT STDMETHODCALLTYPE GetToolTip(IShellItemArray*, LPWSTR* value) override { return GetTitle(nullptr, value); } HRESULT STDMETHODCALLTYPE GetCanonicalName(GUID* value) override { if (!value) return E_POINTER; *value = CLSID_QZipExplorerCommand; return S_OK; } HRESULT STDMETHODCALLTYPE GetState(IShellItemArray*, BOOL, EXPCMDSTATE* state) override { if (!state) return E_POINTER; *state = ECS_ENABLED; return S_OK; } HRESULT STDMETHODCALLTYPE Invoke(IShellItemArray* items, IBindCtx*) override { if (action_ == Action::Root) return S_OK; std::wstring token; return SaveRequest(action_, items, &token) && LaunchQZip(token) ? S_OK : E_FAIL; } HRESULT STDMETHODCALLTYPE GetFlags(EXPCMDFLAGS* flags) override { if (!flags) return E_POINTER; *flags = action_ == Action::Root ? ECF_HASSUBCOMMANDS : ECF_DEFAULT; return S_OK; } HRESULT STDMETHODCALLTYPE EnumSubCommands(IEnumExplorerCommand** value) override { if (action_ != Action::Root) return E_NOTIMPL; *value = new (std::nothrow) CommandEnumerator(); return *value ? S_OK : E_OUTOFMEMORY; }
 private: LONG refs_ = 1; Action action_;
};
CommandEnumerator::CommandEnumerator() { for (Action a : {Action::Open, Action::SevenZip, Action::Zip, Action::ExtractHere, Action::ExtractNamed, Action::MoreOptions}) commands_.push_back(new Command(a)); }
CommandEnumerator::~CommandEnumerator() { for (auto* command : commands_) command->Release(); }
HRESULT CommandEnumerator::QueryInterface(REFIID id, void** out) { if (!out) return E_POINTER; *out = nullptr; if (id == IID_IUnknown || id == IID_IEnumExplorerCommand) { *out = this; AddRef(); return S_OK; } return E_NOINTERFACE; } ULONG CommandEnumerator::AddRef(){return InterlockedIncrement(&refs_);} ULONG CommandEnumerator::Release(){ auto n=InterlockedDecrement(&refs_); if(!n) delete this; return n;} HRESULT CommandEnumerator::Next(ULONG count,IExplorerCommand** out,ULONG* fetched){if(!out)return E_POINTER; ULONG n=0; while(n<count && index_<commands_.size()){out[n]=commands_[index_++];out[n++]->AddRef();}if(fetched)*fetched=n;return n==count?S_OK:S_FALSE;} HRESULT CommandEnumerator::Skip(ULONG count){index_=(std::min)(index_+count,static_cast<ULONG>(commands_.size()));return index_<commands_.size()?S_OK:S_FALSE;} HRESULT CommandEnumerator::Reset(){index_=0;return S_OK;} HRESULT CommandEnumerator::Clone(IEnumExplorerCommand**){return E_NOTIMPL;}
HRESULT Command::QueryInterface(REFIID id, void** out){if(!out)return E_POINTER;*out=nullptr;if(id==IID_IUnknown||id==IID_IExplorerCommand){*out=this;AddRef();return S_OK;}return E_NOINTERFACE;} ULONG Command::AddRef(){return InterlockedIncrement(&refs_);} ULONG Command::Release(){auto n=InterlockedDecrement(&refs_);if(!n)delete this;return n;}
class Factory final : public IClassFactory { public: HRESULT STDMETHODCALLTYPE QueryInterface(REFIID id,void** out) override {if(!out)return E_POINTER;*out=nullptr;if(id==IID_IUnknown||id==IID_IClassFactory){*out=this;AddRef();return S_OK;}return E_NOINTERFACE;} ULONG STDMETHODCALLTYPE AddRef() override{return InterlockedIncrement(&refs_);} ULONG STDMETHODCALLTYPE Release() override{auto n=InterlockedDecrement(&refs_);if(!n)delete this;return n;} HRESULT STDMETHODCALLTYPE CreateInstance(IUnknown* outer,REFIID id,void** out) override{if(outer)return CLASS_E_NOAGGREGATION;auto cmd=new(std::nothrow) Command(Action::Root);if(!cmd)return E_OUTOFMEMORY;auto result=cmd->QueryInterface(id,out);cmd->Release();return result;} HRESULT STDMETHODCALLTYPE LockServer(BOOL) override{return S_OK;} private: LONG refs_=1;};
}
extern "C" HRESULT WINAPI DllCanUnloadNow(){return S_FALSE;} extern "C" HRESULT WINAPI DllGetClassObject(REFCLSID id,REFIID iid,void** out){if(id!=CLSID_QZipExplorerCommand)return CLASS_E_CLASSNOTAVAILABLE;auto factory=new(std::nothrow) Factory();if(!factory)return E_OUTOFMEMORY;auto result=factory->QueryInterface(iid,out);factory->Release();return result;} BOOL WINAPI DllMain(HINSTANCE instance,DWORD reason,LPVOID){if(reason==DLL_PROCESS_ATTACH)g_module=instance;return TRUE;}
