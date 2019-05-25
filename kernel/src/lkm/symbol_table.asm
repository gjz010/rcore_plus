# this reserves space for storing symbol table.
# We just put zero here, and link it in the last.
# 4M is enough. (But too large for Thinpad, so for Thinpad we need better approach.)
.section .data
rcore_symbol_table:
    .zero 1048576
rcore_symbol_table_size:
    .zero 32