from models import Character, Info as InfoAlias, Part
from PySide6.QtWidgets import QPushButton
from PySide6.QtGui import QAction

class Window:
    def __init__(self):
        self.infoI = InfoAlias()
        self.charI = Character()
        self.button = QPushButton()
        self.action = QAction()
        self.ik2fk = self.convert
        self.isSetKey = False

    def convert(self, partI: Part):
        partI.move()

    def run_alias(self, partI: Part):
        self.ik2fk(partI)

    def run_chain(self):
        self.charI.partIs[0].move()

    def read_flag(self):
        return self.isSetKey

    def write_flag(self):
        self.isSetKey = True

    def menuHelp_triggered(self):
        pass

    def wire_released(self):
        self.button.released.connect(self.infoI.selSwCtl_trd)

    def wire_triggered(self):
        self.action.triggered.connect(self.menuHelp_triggered)
        self.button.released.connect(
            self.menuHelp_triggered)

    @property
    def switchValue(self):
        return self.isSetKey

    def read_property(self):
        return self.switchValue


class Child(Window):
    def wire_inherited(self):
        self.action.triggered.connect(self.menuHelp_triggered)


class Ambiguous(Window):
    def __init__(self):
        self.handler = self.first
        self.handler = self.second

    def first(self):
        pass

    def second(self):
        pass

    def run(self):
        self.handler()


class Event:
    def connect(self, callback):
        pass


class Listener:
    def receive(self):
        pass

    def wire(self):
        Event().connect(self.receive)
