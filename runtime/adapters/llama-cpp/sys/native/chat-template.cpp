// Executes the pinned llama.cpp Jinja engine behind a UTF-8-only C boundary.
#include "json.h"
#include "jinja/lexer.h"
#include "jinja/parser.h"
#include "jinja/runtime.h"
#include "jinja/value.h"
#include <cstdlib>
#include <cstring>
#include <exception>
#include <string>
#include <string_view>
#if defined(_WIN32)
#define OID_EXPORT __declspec(dllexport)
#else
#define OID_EXPORT __attribute__((visibility("default")))
#endif
static char * copy_result(std::string_view value, size_t * length) noexcept {
    char * data = static_cast<char *>(std::malloc(value.size() + 1));
    if (!data) { *length = 0; return nullptr; }
    std::memcpy(data, value.data(), value.size());
    data[value.size()] = '\0';
    *length = value.size();
    return data;
}
extern "C" OID_EXPORT char * oid_chat_template_render(const char * source, const char * context, bool * ok, size_t * length) noexcept {
    if (!source || !context || !ok || !length) return nullptr;
    *ok = false;
    *length = 0;
    try {
        jinja::lexer lex;
        auto tokens = lex.tokenize(source);
        auto program = jinja::parse_from_tokens(tokens);
        jinja::context ctx(source);
        auto inputs = common_json::parse(context);
        jinja::global_from_json(ctx, inputs, false);
        jinja::runtime runtime(ctx);
        auto result = jinja::runtime::gather_string_parts(runtime.execute(program))->as_string().str();
        char * data = copy_result(result, length);
        *ok = data != nullptr;
        return data;
    } catch (const std::exception & error) {
        return copy_result(error.what(), length);
    } catch (...) {
        return copy_result("unknown native chat-template error", length);
    }
}
extern "C" OID_EXPORT void oid_chat_template_free(char * data) noexcept { std::free(data); }

extern "C" OID_EXPORT unsigned int oid_chat_template_abi_version() noexcept { return 1; }
