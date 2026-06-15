from __future__ import annotations

from typing import Union

from .models_faults import ParsedFaultArmCommand, ParsedFaultClearCommand, ParsedFaultListCommand
from .models_mailbox import (
    ParsedAckCommand,
    ParsedAskCommand,
    ParsedCancelCommand,
    ParsedInboxCommand,
    ParsedPendCommand,
    ParsedQueueCommand,
    ParsedResubmitCommand,
    ParsedRetryCommand,
    ParsedTraceCommand,
    ParsedWaitCommand,
    ParsedWatchCommand,
)
from .models_start import (
    ParsedClearCommand,
    ParsedCleanupCommand,
    ParsedConfigValidateCommand,
    ParsedDoctorCommand,
    ParsedKillCommand,
    ParsedLogsCommand,
    ParsedMaintenanceCommand,
    ParsedPingCommand,
    ParsedPsCommand,
    ParsedReloadCommand,
    ParsedRestartCommand,
    ParsedStartCommand,
)


ParsedCommand = Union[
    ParsedAckCommand,
    ParsedAskCommand,
    ParsedCancelCommand,
    ParsedClearCommand,
    ParsedCleanupCommand,
    ParsedConfigValidateCommand,
    ParsedDoctorCommand,
    ParsedFaultArmCommand,
    ParsedFaultClearCommand,
    ParsedFaultListCommand,
    ParsedInboxCommand,
    ParsedKillCommand,
    ParsedLogsCommand,
    ParsedMaintenanceCommand,
    ParsedPendCommand,
    ParsedPingCommand,
    ParsedPsCommand,
    ParsedQueueCommand,
    ParsedReloadCommand,
    ParsedRestartCommand,
    ParsedResubmitCommand,
    ParsedRetryCommand,
    ParsedStartCommand,
    ParsedTraceCommand,
    ParsedWaitCommand,
    ParsedWatchCommand,
]
